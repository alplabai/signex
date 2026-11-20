//! Board-feature snaps of a board sketch (#2518, Graphite mockup #179): the
//! cursor reaches the board drawn under the sketch, its outline, pads,
//! holes, vias, track ends and courtyards. Such a snap places the point
//! only and lands no constraint: references to board features are not
//! decided yet. A footprint sketch has no board, so it never snaps here.
//! Rules: `docs/PCB_SKETCH.md`, "Snapping to the board".

use super::snap::{POINT_SNAP_RADIUS_PX, SnapKind, SnapResult};
use super::state::{BoardBackdrop, BoardFeatureSnaps, DEFAULT_PX_PER_MM, FootprintEditorState};

/// What on the board a board-feature snap landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoardFeatureKind {
    /// A vertex of the board outline, or the nearest point on one of its
    /// edges.
    Outline,
    /// A placed pad's centre.
    Pad,
    /// A drilled pad's centre, plated or not.
    Hole,
    /// A via's centre.
    Via,
    /// Either end of a track.
    TrackEnd,
    /// A corner of a component courtyard.
    Courtyard,
}

impl BoardFeatureKind {
    /// Every kind, in Snap ▾ order.
    pub const ALL: [Self; 6] = [
        Self::Outline,
        Self::Pad,
        Self::Hole,
        Self::Via,
        Self::TrackEnd,
        Self::Courtyard,
    ];
}

impl BoardFeatureSnaps {
    /// Whether a snap to `kind` may fire: board features on, and that kind.
    pub fn allows(&self, kind: BoardFeatureKind) -> bool {
        self.enabled && self.kind_on(kind)
    }

    /// The per-kind switch alone, as its Snap ▾ row shows it.
    pub fn kind_on(&self, kind: BoardFeatureKind) -> bool {
        match kind {
            BoardFeatureKind::Outline => self.outline,
            BoardFeatureKind::Pad => self.pads,
            BoardFeatureKind::Hole => self.holes,
            BoardFeatureKind::Via => self.vias,
            BoardFeatureKind::TrackEnd => self.track_ends,
            BoardFeatureKind::Courtyard => self.courtyards,
        }
    }

    /// Flip the per-kind switch of `kind`.
    pub fn toggle(&mut self, kind: BoardFeatureKind) {
        let flag = match kind {
            BoardFeatureKind::Outline => &mut self.outline,
            BoardFeatureKind::Pad => &mut self.pads,
            BoardFeatureKind::Hole => &mut self.holes,
            BoardFeatureKind::Via => &mut self.vias,
            BoardFeatureKind::TrackEnd => &mut self.track_ends,
            BoardFeatureKind::Courtyard => &mut self.courtyards,
        };
        *flag = !*flag;
    }
}

/// The board-feature snap at `raw`, or `None` on a footprint sketch, with
/// board features off, or with no feature in reach. The reach is the
/// sketch Point snap's, in screen pixels at the current zoom.
pub fn board_feature_snap(raw: (f64, f64), state: &FootprintEditorState) -> Option<SnapResult> {
    let backdrop = state.board_backdrop.as_deref()?;
    let snaps = state.snap_options.board_features;
    if !snaps.enabled {
        return None;
    }
    let reach = reach_mm(state.view_px_per_mm);
    nearest_feature(backdrop, snaps, raw, reach).map(|(pos, kind)| SnapResult {
        pos,
        kind: SnapKind::BoardFeature(kind),
    })
}

/// [`POINT_SNAP_RADIUS_PX`] in board mm at `px_per_mm`; a zoom not yet
/// reported reads as the default one.
pub fn reach_mm(px_per_mm: f32) -> f64 {
    let scale = if px_per_mm.is_finite() && px_per_mm > 0.0 {
        px_per_mm
    } else {
        DEFAULT_PX_PER_MM
    };
    f64::from(POINT_SNAP_RADIUS_PX / scale)
}

/// The nearest allowed feature within `reach` mm of `raw`. A point feature
/// (a vertex, a centre, a track end, a courtyard corner) wins over an
/// outline edge; between equally near points the first in
/// [`BoardFeatureKind::ALL`] order wins, so a through-hole pad reads as a
/// pad rather than its hole.
pub fn nearest_feature(
    backdrop: &BoardBackdrop,
    snaps: BoardFeatureSnaps,
    raw: (f64, f64),
    reach: f64,
) -> Option<((f64, f64), BoardFeatureKind)> {
    use BoardFeatureKind as K;
    let mut nearest = Nearest::new(snaps, raw, reach);
    for &p in &backdrop.outline {
        nearest.offer(p, K::Outline);
    }
    for pad in &backdrop.pads {
        nearest.offer(pad.centre, K::Pad);
    }
    for &(centre, _) in &backdrop.holes {
        nearest.offer(centre, K::Hole);
    }
    for &(centre, _) in &backdrop.vias {
        nearest.offer(centre, K::Via);
    }
    for track in &backdrop.tracks {
        nearest.offer(track.from, K::TrackEnd);
        nearest.offer(track.to, K::TrackEnd);
    }
    for courtyard in &backdrop.courtyards {
        for &p in &courtyard.points {
            nearest.offer(p, K::Courtyard);
        }
    }
    if nearest.best.is_none() {
        let n = backdrop.outline.len();
        for (i, &a) in backdrop.outline.iter().enumerate() {
            let b = backdrop.outline[(i + 1) % n];
            nearest.offer(closest_on_segment(a, b, raw), K::Outline);
        }
    }
    nearest.best.map(|(_, pos, kind)| (pos, kind))
}

/// The name the snap badge gives the feature at `pos`: a pad's designator
/// (a plated hole takes its pad's), a courtyard's component; `None` for
/// the rest and for a feature with no name.
pub fn feature_name(
    backdrop: &BoardBackdrop,
    kind: BoardFeatureKind,
    pos: (f64, f64),
) -> Option<String> {
    let name = match kind {
        BoardFeatureKind::Pad | BoardFeatureKind::Hole => {
            backdrop.pad_at(pos).map(|p| p.designator.clone())
        }
        BoardFeatureKind::Courtyard => backdrop.courtyard_at(pos).map(|c| c.reference.clone()),
        BoardFeatureKind::Outline | BoardFeatureKind::Via | BoardFeatureKind::TrackEnd => None,
    };
    name.filter(|n| !n.is_empty())
}

/// The running nearest candidate of [`nearest_feature`].
struct Nearest {
    snaps: BoardFeatureSnaps,
    raw: (f64, f64),
    reach_sq: f64,
    best: Option<(f64, (f64, f64), BoardFeatureKind)>,
}

impl Nearest {
    fn new(snaps: BoardFeatureSnaps, raw: (f64, f64), reach: f64) -> Self {
        Self {
            snaps,
            raw,
            reach_sq: reach * reach,
            best: None,
        }
    }

    fn offer(&mut self, p: (f64, f64), kind: BoardFeatureKind) {
        if !self.snaps.allows(kind) || !p.0.is_finite() || !p.1.is_finite() {
            return;
        }
        let d_sq = (p.0 - self.raw.0).powi(2) + (p.1 - self.raw.1).powi(2);
        if d_sq > self.reach_sq {
            return;
        }
        if self.best.is_none_or(|(best_sq, _, _)| d_sq < best_sq) {
            self.best = Some((d_sq, p, kind));
        }
    }
}

/// The point of segment `a`-`b` nearest `p`.
fn closest_on_segment(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-18 {
        return a;
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0);
    (a.0 + t * dx, a.1 + t * dy)
}
