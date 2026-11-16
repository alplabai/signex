//! Glossing (#2408): the selected routes made shorter and smoother by
//! cutting their corners. Where exactly two selected tracks of one net,
//! layer and width meet at a free corner (no other track end, pad or via
//! there), the two become one straight track between their far ends, when
//! that track keeps the clearance to copper of other nets and stays out of
//! keepouts that forbid routing. Repeated until no corner can go. The ends
//! of a route never move, so nothing it connects comes loose. Rules:
//! `docs/PCB_TOOLS.md`, "Gloss".

use std::collections::{BTreeMap, BTreeSet};

use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

use crate::copper::{layer_mask, pad_layers, via_layers};
use crate::index::{ItemRef, track_shape, via_shape};
use crate::obstacle::{KeepoutArea, NewCopper, Obstacle, clashes, copper_obstacles, keepout_areas};
use crate::transform::pad_shape;
use crate::units::PointNm;

/// What [`gloss`] changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Glossed {
    /// Tracks kept with new end points, as they are now.
    pub replaced: Vec<Segment>,
    /// Tracks merged into a neighbour.
    pub removed: Vec<Uuid>,
}

impl Glossed {
    pub fn is_empty(&self) -> bool {
        self.replaced.is_empty() && self.removed.is_empty()
    }
}

/// The board as glossing reads it while it works.
struct Work<'a, F> {
    board: &'a PcbBoard,
    /// The selected tracks as they are now.
    tracks: Vec<Segment>,
    /// Selected tracks merged away.
    gone: BTreeSet<Uuid>,
    obstacles: Vec<Obstacle>,
    keepouts: Vec<KeepoutArea>,
    clearance: F,
}

/// One corner cut: the track kept, the track merged into it, and the kept
/// track as it becomes.
struct Cut {
    keep: usize,
    drop: usize,
    joined: Segment,
}

/// Gloss the tracks `selected` names on `board`. `clearance(a, b)` is the gap
/// nets `a` and `b` need between them. Deterministic: corners are tried in
/// order of their layer and position.
pub fn gloss<F: Fn(u32, u32) -> i64>(
    board: &PcbBoard,
    selected: &BTreeSet<Uuid>,
    clearance: F,
) -> Glossed {
    let tracks = board
        .segments
        .iter()
        .filter(|s| selected.contains(&s.uuid))
        .filter(|s| layer_mask(std::iter::once(s.layer.as_str())) != 0)
        .filter(|s| track_shape(s).is_ok())
        .cloned()
        .collect();
    let mut work = Work {
        board,
        tracks,
        gone: BTreeSet::new(),
        obstacles: copper_obstacles(board),
        keepouts: keepout_areas(board),
        clearance,
    };
    while let Some(cut) = work.next_cut() {
        work.apply(cut);
    }
    let replaced = work
        .tracks
        .into_iter()
        .filter(|s| {
            board
                .segments
                .iter()
                .any(|old| old.uuid == s.uuid && old != s)
        })
        .collect();
    Glossed {
        replaced,
        removed: work.gone.into_iter().collect(),
    }
}

impl<F: Fn(u32, u32) -> i64> Work<'_, F> {
    fn apply(&mut self, cut: Cut) {
        let gone = self.tracks[cut.drop].uuid;
        let kept = cut.joined.uuid;
        if let (Ok(shape), Some(slot)) = (
            track_shape(&cut.joined),
            self.obstacles
                .iter_mut()
                .find(|o| o.item == ItemRef::Track(kept)),
        ) {
            slot.shape = shape;
        }
        self.obstacles.retain(|o| o.item != ItemRef::Track(gone));
        self.tracks[cut.keep] = cut.joined;
        self.tracks.remove(cut.drop);
        self.gone.insert(gone);
    }

    /// The first corner that can be cut.
    fn next_cut(&self) -> Option<Cut> {
        let ends = self.end_counts();
        let mut corners: BTreeMap<(String, PointNm), Vec<usize>> = BTreeMap::new();
        for (i, s) in self.tracks.iter().enumerate() {
            for (p, _) in ends_of(s) {
                corners.entry((s.layer.clone(), p)).or_default().push(i);
            }
        }
        corners.into_iter().find_map(|((layer, at), meeting)| {
            let [keep, drop] = meeting[..] else {
                return None;
            };
            let free = ends.get(&(layer.clone(), at)).copied() == Some(2);
            free.then(|| self.cut_at(&layer, at, keep, drop)).flatten()
        })
    }

    /// The cut at the corner `at` where tracks `keep` and `drop` meet, when
    /// the joined track would be clear.
    fn cut_at(&self, layer: &str, at: PointNm, keep: usize, drop: usize) -> Option<Cut> {
        let (a, b) = (&self.tracks[keep], &self.tracks[drop]);
        if keep == drop || a.net != b.net || a.width != b.width || self.pinned(layer, at) {
            return None;
        }
        let (from, _) = far_end(a, at)?;
        let (to_nm, to) = far_end(b, at)?;
        if from == to_nm {
            return None;
        }
        let joined = rejoined(a, at, to);
        let shape = track_shape(&joined).ok()?;
        let layers = layer_mask(std::iter::once(layer));
        let merged = [ItemRef::Track(a.uuid), ItemRef::Track(b.uuid)];
        let blocked = clashes(
            &shape,
            NewCopper::Route,
            a.net,
            layers,
            &self.obstacles,
            &self.clearance,
            |item| merged.contains(&item),
        ) || self
            .keepouts
            .iter()
            .any(|k| k.bars(NewCopper::Route, layers, &shape));
        (!blocked).then_some(Cut { keep, drop, joined })
    }

    /// How many track ends of the board, the glossed ones as they are now,
    /// lie at each point of each layer.
    fn end_counts(&self) -> BTreeMap<(String, PointNm), usize> {
        let mut counts = BTreeMap::new();
        let untouched = self.board.segments.iter().filter(|s| {
            !self.gone.contains(&s.uuid) && !self.tracks.iter().any(|w| w.uuid == s.uuid)
        });
        for s in untouched.chain(self.tracks.iter()) {
            for (p, _) in ends_of(s) {
                *counts.entry((s.layer.clone(), p)).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Whether a pad or via of any net holds `at` on `layer`: a corner there
    /// is a connection, not a bend.
    fn pinned(&self, layer: &str, at: PointNm) -> bool {
        let layers = layer_mask(std::iter::once(layer));
        let board = self.board;
        let in_via = board.vias.iter().any(|via| {
            via_layers(via) & layers != 0 && via_shape(via).is_ok_and(|s| s.contains(at, 0))
        });
        in_via
            || board.footprints.iter().any(|footprint| {
                footprint.pads.iter().any(|pad| {
                    pad_layers(footprint, pad) & layers != 0
                        && pad_shape(footprint, pad).is_ok_and(|s| s.contains(at, 0))
                })
            })
    }
}

/// The ends of `s` in nm, with the model point each came from.
fn ends_of(s: &Segment) -> Vec<(PointNm, Point)> {
    [s.start, s.end]
        .into_iter()
        .filter_map(|p| PointNm::from_mm(p, "track end").ok().map(|nm| (nm, p)))
        .collect()
}

/// The end of `s` that is not `at`.
fn far_end(s: &Segment, at: PointNm) -> Option<(PointNm, Point)> {
    match ends_of(s)[..] {
        [p, q] if p.0 == at => Some(q),
        [p, q] if q.0 == at => Some(p),
        _ => None,
    }
}

/// `s` with its end at `at` moved to `to`: the uuid, width, layer and net
/// stay, and so does which field holds which end.
fn rejoined(s: &Segment, at: PointNm, to: Point) -> Segment {
    let mut joined = s.clone();
    if PointNm::from_mm(s.start, "track.start").ok() == Some(at) {
        joined.start = to;
    } else {
        joined.end = to;
    }
    joined
}

#[cfg(test)]
#[path = "gloss_tests.rs"]
mod tests;
