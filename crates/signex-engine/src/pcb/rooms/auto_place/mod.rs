//! Auto Place in rooms (#2941): the unlocked members of each room put
//! inside it one at a time, each where the weighted score of four
//! priorities (connection length, thermal spacing, noisy and sensitive
//! separation, connectors on edges) is lowest. Placement only: nothing is
//! routed and no part turns or changes side. Pure and deterministic: the
//! same board, rooms and weights give the same placement. Locked parts and
//! the parts of rooms not placed stay and are in the way. Rules:
//! `docs/PCB_ROOMS.md`, "Auto place".

mod input;
mod score;

#[cfg(test)]
mod tests;

pub use input::{PLACEMENT_PROPERTY, PartRoles, Priority, Weights};
pub use score::{Gaps, Scores};

use pcb_geom::{PointNm, RectNm, Side, placed_extent};
use signex_types::pcb::{Footprint, PcbBoard, Point, Room};
use uuid::Uuid;

use super::arrange::placed_box;
use super::inner_box;
use super::plan::board_region;
use crate::pcb::BoardCommand;
use crate::pcb::board_tools::PackGrid;
use score::{Field, Traits, board_scores};

/// How far hot parts keep apart when no gap is given, mm.
pub const DEFAULT_THERMAL_GAP_MM: f64 = 5.0;
/// How far a noisy part keeps from a sensitive one when no gap is given, mm.
pub const DEFAULT_SEPARATION_GAP_MM: f64 = 10.0;

/// The least distance between two places tried, mm.
const MIN_PITCH_NM: i64 = 500_000;
/// The most places tried for one part: the pitch doubles until they fit.
const MAX_CANDIDATES: usize = 4096;

/// What Auto Place is asked to do.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoPlaceOptions {
    /// The rooms to place, by uuid; empty places every room.
    pub rooms: Vec<Uuid>,
    pub weights: Weights,
    pub gaps: Gaps,
}

impl Default for AutoPlaceOptions {
    fn default() -> Self {
        AutoPlaceOptions {
            rooms: Vec::new(),
            weights: Weights::default(),
            gaps: Gaps {
                thermal_mm: DEFAULT_THERMAL_GAP_MM,
                separation_mm: DEFAULT_SEPARATION_GAP_MM,
            },
        }
    }
}

/// A part Auto Place moves.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedPart {
    pub footprint: Uuid,
    pub designator: String,
    /// The name of the room it was placed in.
    pub room: String,
    pub from: Point,
    pub to: Point,
    /// Unchanged: Auto Place does not turn a part.
    pub rotation: f64,
    /// Unchanged: Auto Place does not flip a part.
    pub bottom: bool,
}

/// Why a part stayed where it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotPlacedReason {
    /// No place inside the room is free of the other parts and keepouts.
    NoSpace,
    /// Its body, or the room's outline, does not convert to board geometry.
    Unreadable,
}

/// A part Auto Place could not place: it stays where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct NotPlaced {
    pub footprint: Uuid,
    pub designator: String,
    pub room: String,
    pub reason: NotPlacedReason,
}

/// What Auto Place does.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutoPlacePlan {
    /// The rooms placed, by name, in board order.
    pub rooms: Vec<String>,
    /// The parts that move, in the order they were placed.
    pub moves: Vec<PlacedPart>,
    pub not_placed: Vec<NotPlaced>,
    /// The score of each priority over the parts placed, before and after.
    pub before: Scores,
    pub after: Scores,
}

impl AutoPlacePlan {
    /// The moves as board commands, to run as one undo step.
    pub fn commands(&self) -> Vec<BoardCommand> {
        self.moves
            .iter()
            .map(|m| BoardCommand::MoveFootprint {
                footprint: m.footprint,
                to: m.to,
            })
            .collect()
    }
}

/// The room of `rooms` that places `fp`: the deepest sheet that holds it,
/// the first in board order of equals. Locked parts have none.
fn owner_of(fp: &Footprint, rooms: &[&Room]) -> Option<usize> {
    if fp.locked {
        return None;
    }
    let depth = |room: &Room| room.source.as_ref().map_or(0, |s| s.sheet.chain().len());
    rooms
        .iter()
        .enumerate()
        .filter(|(_, room)| room.holds(fp))
        .fold(None, |best: Option<(usize, usize)>, (k, room)| match best {
            Some((_, d)) if d >= depth(*room) => best,
            _ => Some((k, depth(*room))),
        })
        .map(|(k, _)| k)
}

fn traits_of(fp: &Footprint) -> Traits {
    let mut nets: Vec<u32> = fp
        .pads
        .iter()
        .filter_map(|pad| pad.net.as_ref().map(|n| n.number))
        .filter(|n| *n != 0)
        .collect();
    nets.sort_unstable();
    nets.dedup();
    Traits {
        nets,
        roles: PartRoles::of(fp),
    }
}

/// The boxes of the keepouts that forbid components, on either side.
fn component_keepouts(board: &PcbBoard) -> Vec<RectNm> {
    let drawn: Vec<_> = board.zones.iter().filter_map(|z| z.as_keepout()).collect();
    let sketch = board.sketch.iter().flat_map(|s| s.keepouts.iter());
    sketch
        .chain(&drawn)
        .filter(|k| k.forbids.no_components)
        .filter_map(|k| {
            let corners: Option<Vec<PointNm>> = k
                .boundary
                .iter()
                .map(|p| PointNm::from_mm(*p, "keepout").ok())
                .collect();
            RectNm::bounding(corners?)
        })
        .collect()
}

/// Whether the two boxes share area; boxes that only touch do not.
fn overlaps(a: &RectNm, b: &RectNm) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

/// The grid values from `lo` to `hi`, `pitch` apart, counted from 0.
fn steps(lo: i64, hi: i64, pitch: i64) -> Vec<i64> {
    if lo > hi {
        return Vec::new();
    }
    let first = lo.div_euclid(pitch) * pitch;
    let first = if first < lo { first + pitch } else { first };
    (0..)
        .map(|k| first + k * pitch)
        .take_while(|v| *v <= hi)
        .collect()
}

/// The search for one part's place: what it is scored against, what is in
/// its way and the grid its origin stays on.
struct Search<'a> {
    field: &'a Field<'a>,
    weights: &'a Weights,
    sides: &'a [Side],
    keepouts: &'a [RectNm],
    grid: PackGrid,
}

impl Search<'_> {
    /// Where part `i` goes inside `area`: the origin, on the components
    /// grid, of the lowest weighted score, the first in rows from the
    /// top-left among equals; `None` when no place is free.
    fn best(&self, i: usize, extent: RectNm, area: RectNm) -> Option<PointNm> {
        let gap = self.grid.gap;
        // Origins in grid space whose box stays `gap` inside the area.
        let local = self.grid.to_grid(area);
        let (x0, x1) = (
            local.min.x + gap - extent.min.x,
            local.max.x - gap - extent.max.x,
        );
        let (y0, y1) = (
            local.min.y + gap - extent.min.y,
            local.max.y - gap - extent.max.y,
        );
        let step = self.grid.step.max(1);
        let mut pitch = step * ((MIN_PITCH_NM + step - 1) / step).max(1);
        let (mut xs, mut ys) = (steps(x0, x1, pitch), steps(y0, y1, pitch));
        while xs.len() * ys.len() > MAX_CANDIDATES {
            pitch *= 2;
            (xs, ys) = (steps(x0, x1, pitch), steps(y0, y1, pitch));
        }
        let mut best: Option<(f64, PointNm)> = None;
        for &y in &ys {
            for &x in &xs {
                let origin = self.grid.to_board_nm(PointNm::new(x, y));
                let at = RectNm::new(origin.plus(extent.min), origin.plus(extent.max));
                if !self.is_free(i, &at) {
                    continue;
                }
                let score = self.field.place_score(i, &at).total(self.weights);
                if best.is_none_or(|(s, _)| score < s - 1e-9) {
                    best = Some((score, origin));
                }
            }
        }
        best.map(|(_, origin)| origin)
    }

    /// Whether `at` keeps the gap from every settled part on the same side
    /// and stays out of every component keepout.
    fn is_free(&self, i: usize, at: &RectNm) -> bool {
        let kept = at.inflate(self.grid.gap);
        let field = self.field;
        let clear_of_parts = field.boxes.iter().enumerate().all(|(j, other)| {
            j == i
                || !field.settled[j]
                || self.sides[j] != self.sides[i]
                || other.as_ref().is_none_or(|o| !overlaps(&kept, o))
        });
        clear_of_parts && self.keepouts.iter().all(|k| !overlaps(at, k))
    }
}

/// Auto Place on `board` with `options`.
pub fn auto_place(board: &PcbBoard, options: &AutoPlaceOptions) -> AutoPlacePlan {
    let fps = &board.footprints;
    let rooms: Vec<&Room> = board
        .rooms
        .iter()
        .filter(|r| options.rooms.is_empty() || options.rooms.contains(&r.uuid))
        .collect();
    let traits: Vec<Traits> = fps.iter().map(traits_of).collect();
    let sides: Vec<Side> = fps.iter().map(|fp| Side::of_layer(&fp.layer)).collect();
    let mut boxes: Vec<Option<RectNm>> = fps.iter().map(placed_box).collect();
    let owners: Vec<Option<usize>> = fps.iter().map(|fp| owner_of(fp, &rooms)).collect();
    let counted: Vec<bool> = owners.iter().map(Option::is_some).collect();
    let region = board_region(board);
    let gaps = options.gaps;
    let mut plan = AutoPlacePlan {
        rooms: rooms.iter().map(|r| r.name.clone()).collect(),
        before: board_scores(&traits, &boxes, &counted, &region, &gaps),
        ..AutoPlacePlan::default()
    };
    let keepouts = component_keepouts(board);
    let grid = PackGrid::of(board);
    let degree: Vec<usize> = traits
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let others = traits.iter().enumerate().filter(|(j, _)| *j != i);
            others.map(|(_, o)| t.shared(o)).sum::<usize>()
        })
        .collect();
    let mut settled: Vec<bool> = counted.iter().map(|c| !c).collect();
    for (k, room) in rooms.iter().enumerate() {
        let mut order: Vec<usize> = (0..fps.len()).filter(|i| owners[*i] == Some(k)).collect();
        let area_of =
            |i: usize| boxes[i].map_or(0i128, |b| i128::from(b.width()) * i128::from(b.height()));
        order.sort_by(|&a, &b| {
            degree[b]
                .cmp(&degree[a])
                .then_with(|| area_of(b).cmp(&area_of(a)))
                .then_with(|| fps[a].reference.cmp(&fps[b].reference))
                .then_with(|| fps[a].uuid.cmp(&fps[b].uuid))
        });
        let area = inner_box(room).and_then(|b| b.as_rect());
        for i in order {
            let fp = &fps[i];
            let extent = placed_extent(fp).ok();
            let found = match (area, grid, extent) {
                (Some(area), Some(grid), Some(extent)) => {
                    let field = Field {
                        traits: &traits,
                        boxes: &boxes,
                        settled: &settled,
                        board: &region,
                        gaps: &gaps,
                    };
                    let search = Search {
                        field: &field,
                        weights: &options.weights,
                        sides: &sides,
                        keepouts: &keepouts,
                        grid,
                    };
                    search
                        .best(i, extent, area)
                        .map(|origin| (origin, extent))
                        .ok_or(NotPlacedReason::NoSpace)
                }
                _ => Err(NotPlacedReason::Unreadable),
            };
            settled[i] = true;
            match found {
                Ok((origin, extent)) => {
                    boxes[i] = Some(RectNm::new(
                        origin.plus(extent.min),
                        origin.plus(extent.max),
                    ));
                    let to = origin.to_mm();
                    if to != fp.position {
                        plan.moves.push(PlacedPart {
                            footprint: fp.uuid,
                            designator: fp.reference.clone(),
                            room: room.name.clone(),
                            from: fp.position,
                            to,
                            rotation: fp.rotation,
                            bottom: sides[i] == Side::Bottom,
                        });
                    }
                }
                Err(reason) => plan.not_placed.push(NotPlaced {
                    footprint: fp.uuid,
                    designator: fp.reference.clone(),
                    room: room.name.clone(),
                    reason,
                }),
            }
        }
    }
    plan.after = board_scores(&traits, &boxes, &counted, &region, &gaps);
    plan
}
