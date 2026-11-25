//! The edit that fixes one finding (#2940): what each rule kind may change,
//! the locks that keep an item out, and the geometry of a move that clears
//! a gap.

use pcb_geom::{
    ItemRef, PointNm, Separation, Shape, collect_copper, nm_to_mm, no_net_copper, pad_shape,
    pad_world_center,
};
use signex_drc::Violation;
use signex_types::object_id::IdSource;
use signex_types::pcb::{Footprint, Pad, PcbBoard, Point};
use signex_types::pcb_free::free_pad_frame;

use super::{FixRule, LeftReason, mask, moves, sizes};
use crate::pcb::{BoardCommand, BoardItem, BoardItemId, lock, placement};

/// The longest move a fix makes, in micrometres.
const MAX_MOVE_UM: i64 = 10_000;

/// One way to fix a finding.
pub(super) struct Plan {
    /// The item the fix moves or changes; a later attempt keeps to it.
    pub(super) subject: String,
    pub(super) commands: Vec<BoardCommand>,
    /// The items the commands change, as an answer names them.
    pub(super) changed: Vec<String>,
}

impl Plan {
    /// Put `items` over the board items with their ids; the first one is
    /// the subject.
    pub(super) fn replacing(items: Vec<BoardItem>) -> Plan {
        let changed: Vec<String> = items.iter().map(|i| i.id().uuid().to_string()).collect();
        Plan {
            subject: changed.first().cloned().unwrap_or_default(),
            commands: vec![BoardCommand::ReplaceItems { items }],
            changed,
        }
    }

    /// `commands`, which move or change `subject` and what follows it.
    pub(super) fn of_commands(subject: String, commands: Vec<BoardCommand>) -> Plan {
        let mut changed = vec![subject.clone()];
        for command in &commands {
            let ids: Vec<String> = match command {
                BoardCommand::ReplaceItems { items } | BoardCommand::AddItems { items } => {
                    items.iter().map(|i| i.id().uuid().to_string()).collect()
                }
                BoardCommand::ReplacePads { pads } => pads
                    .iter()
                    .map(|p| format!("{}/{}", p.footprint, p.pad.uuid))
                    .collect(),
                _ => Vec::new(),
            };
            for id in ids {
                if !changed.contains(&id) {
                    changed.push(id);
                }
            }
        }
        Plan {
            subject,
            commands,
            changed,
        }
    }
}

/// The ways to fix `v` on `board`, the first to try first; `Err` when there
/// is none.
pub(super) fn plans(
    board: &PcbBoard,
    rule: FixRule,
    v: &Violation,
    new_ids: &mut dyn IdSource,
) -> Result<Vec<Plan>, LeftReason> {
    match rule {
        FixRule::Clearance => moves::clearance(board, v, new_ids),
        FixRule::HoleToHole => moves::hole_to_hole(board, v, new_ids),
        FixRule::SilkToPad => moves::silk_to_pad(board, v),
        FixRule::TrackWidth => sizes::track_width(board, v),
        FixRule::AnnularRing => sizes::annular_ring(board, v),
        FixRule::MaskSliver => mask::mask_sliver(board, v),
    }
}

/// `plans`, or why there is none: the locked items when a lock kept every
/// way out, else `otherwise`.
pub(super) fn found(
    plans: Vec<Plan>,
    locked: Vec<String>,
    otherwise: &'static str,
) -> Result<Vec<Plan>, LeftReason> {
    if !plans.is_empty() {
        Ok(plans)
    } else if !locked.is_empty() {
        Err(LeftReason::Locked(locked))
    } else {
        Err(LeftReason::CannotChange(otherwise))
    }
}

/// How far short of its limit `v`'s gap or size is, nm.
pub(super) fn shortfall(v: &Violation) -> i64 {
    (v.required_nm - v.measured_nm).max(0)
}

/// `nm` in mm, rounded up to a whole micrometre, plus one: a change that
/// still passes the limit once the board stores it in nm.
pub(super) fn step_mm(nm: i64) -> f64 {
    ((nm.max(0) + 999) / 1000 + 1) as f64 / 1000.0
}

/// A footprint whose pads a fix leaves alone: locked in place, or its pads
/// and drawings locked.
pub(super) fn part_locked(footprint: &Footprint) -> bool {
    footprint.locked || footprint.component.lock_primitives
}

/// How a refusal names `item` when a lock keeps it out; `None` when it is
/// free to change.
pub(super) fn locked_name(board: &PcbBoard, item: ItemRef) -> Option<String> {
    let id = match item {
        ItemRef::Track(id) => BoardItemId::Segment(id),
        ItemRef::Via(id) => BoardItemId::Via(id),
        ItemRef::Arc(id) => BoardItemId::Arc(id),
        ItemRef::Pad { footprint, .. } => {
            let fp = board.footprints.iter().find(|f| f.uuid == footprint)?;
            return part_locked(fp).then(|| format!("footprint {}", placement::name(fp)));
        }
        _ => return None,
    };
    lock::locked_names(board, [id]).into_iter().next()
}

/// The copper of `item` on `layers` (every layer for 0).
pub(super) fn copper_shapes(board: &PcbBoard, item: ItemRef, layers: u32) -> Vec<Shape> {
    let layers = if layers == 0 { u32::MAX } else { layers };
    collect_copper(board)
        .into_iter()
        .chain(no_net_copper(board))
        .filter(|c| c.item == item && c.layers & layers != 0)
        .map(|c| c.shape)
        .collect()
}

/// The pad `item` names: one of a footprint, or a free pad.
pub(super) fn pad_of(board: &PcbBoard, item: ItemRef) -> Option<&Pad> {
    match item {
        ItemRef::Pad { footprint, pad } => board
            .footprints
            .iter()
            .find(|f| f.uuid == footprint)?
            .pads
            .iter()
            .find(|p| p.uuid == pad),
        ItemRef::FreePad(id) => board.free.pads.iter().find(|p| p.uuid == id),
        _ => None,
    }
}

/// The copper shape and board position of the pad `item` names.
pub(super) fn pad_geometry(board: &PcbBoard, item: ItemRef) -> Option<(Shape, Point)> {
    let of = |fp: &Footprint, pad: &Pad| {
        let shape = pad_shape(fp, pad).ok()?;
        Some((shape, pad_world_center(fp, pad).ok()?.to_mm()))
    };
    match item {
        ItemRef::Pad { footprint, pad } => {
            let fp = board.footprints.iter().find(|f| f.uuid == footprint)?;
            of(fp, fp.pads.iter().find(|p| p.uuid == pad)?)
        }
        ItemRef::FreePad(id) => {
            let pad = board.free.pads.iter().find(|p| p.uuid == id)?;
            of(&free_pad_frame(), pad)
        }
        _ => None,
    }
}

/// `(dx, dy)` scaled to length 1; `None` for no length.
pub(super) fn unit(dx: f64, dy: f64) -> Option<Point> {
    let len = dx.hypot(dy);
    (len.is_finite() && len > 0.0).then(|| Point::new(dx / len, dy / len))
}

/// `dir` scaled to `mm`.
pub(super) fn scaled(dir: Point, mm: f64) -> Point {
    Point::new(dir.x * mm, dir.y * mm)
}

/// The middle of the bounds of the first of `shapes`, mm.
fn centre(shapes: &[Shape]) -> Option<Point> {
    let bounds = shapes.first()?.aabb();
    Some(Point::new(
        nm_to_mm((bounds.min.x + bounds.max.x) / 2),
        nm_to_mm((bounds.min.y + bounds.max.y) / 2),
    ))
}

/// The direction that takes `mover` away from `other`: from the nearest
/// point of `other` to the nearest point of `mover`, or from centre to
/// centre when they touch (the nearest points mean nothing then).
pub(super) fn away(mover: &[Shape], other: &[Shape]) -> Option<Point> {
    let by_centre = || {
        let (from, to) = (centre(other)?, centre(mover)?);
        unit(to.x - from.x, to.y - from.y)
    };
    let mut nearest: Option<Separation> = None;
    for a in mover {
        for b in other {
            if a.overlaps(b) {
                return by_centre();
            }
            let s = a.separation(b);
            if nearest.is_none_or(|n| s.distance_nm() < n.distance_nm()) {
                nearest = Some(s);
            }
        }
    }
    let (a, b) = nearest?.between;
    unit((a.x - b.x) as f64, (a.y - b.y) as f64).or_else(by_centre)
}

/// How far `mover` must go along `dir` (length 1) for each of its shapes to
/// be at least `limit` nm from each of `other`'s: mm, rounded up to a whole
/// micrometre, plus one. `None` when it is clear already, or when no move
/// up to [`MAX_MOVE_UM`] clears it.
pub(super) fn clearing_move(
    mover: &[Shape],
    other: &[Shape],
    dir: Point,
    limit: i64,
) -> Option<f64> {
    let clear = |um: i64| {
        let nm = um as f64 * 1000.0;
        let by = PointNm::new((dir.x * nm).round() as i64, (dir.y * nm).round() as i64);
        mover.iter().all(|a| {
            let moved = a.clone().translated(by);
            other
                .iter()
                .all(|b| !moved.overlaps(b) && !moved.separation(b).is_under(limit))
        })
    };
    if mover.is_empty() || other.is_empty() || clear(0) || !clear(MAX_MOVE_UM) {
        return None;
    }
    let (mut lo, mut hi) = (0, MAX_MOVE_UM);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if clear(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some((hi + 1) as f64 / 1000.0)
}
