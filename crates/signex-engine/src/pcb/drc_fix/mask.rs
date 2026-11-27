//! The solder mask sliver fix (#2940): widen the strip of mask past the
//! rule by shrinking the opening of a via or track, or, between two
//! openings of one net, merge them by growing one until they meet. A pad's
//! opening comes from its footprint and is left alone.

use pcb_geom::ItemRef;
use signex_drc::Violation;
use signex_types::pcb::{MaskExpansion, PcbBoard};

use super::LeftReason;
use super::plan::{Plan, found, locked_name, shortfall, step_mm};
use crate::pcb::BoardItem;

/// Widen the strip first; merge only openings of one net, so no copper of
/// two nets is bared side by side.
pub(super) fn mask_sliver(board: &PcbBoard, v: &Violation) -> Result<Vec<Plan>, LeftReason> {
    let widen = -step_mm(shortfall(v));
    let merge = step_mm(v.measured_nm);
    let one_net = v.nets.0 == v.nets.1 && v.nets.0 != 0;
    let mut grows = vec![widen];
    if one_net {
        grows.push(merge);
    }
    let (mut plans, mut locked) = (Vec::new(), Vec::new());
    for by in grows {
        for &item in &v.items {
            if !matches!(item, ItemRef::Via(_) | ItemRef::Track(_)) {
                continue;
            }
            if let Some(name) = locked_name(board, item) {
                if !locked.contains(&name) {
                    locked.push(name);
                }
                continue;
            }
            if let Some(opening) = opening_grown(board, item, by) {
                let mut plan = Plan::replacing(vec![opening]);
                // A later attempt keeps to the same way, widen or merge.
                if by > 0.0 {
                    plan.subject.push_str("#merge");
                }
                plans.push(plan);
            }
        }
    }
    found(
        plans,
        locked,
        "both openings are pads, whose mask openings come from their footprints",
    )
}

/// The via or track `item` with its mask opening grown by `by` mm (shrunk
/// for a negative `by`) on each side it is open; `None` when it has no
/// opening.
fn opening_grown(board: &PcbBoard, item: ItemRef, by: f64) -> Option<BoardItem> {
    let grow = |e: MaskExpansion| MaskExpansion::Manual {
        mm: e.manual_mm().unwrap_or(0.0) + by,
    };
    match item {
        ItemRef::Via(id) => {
            let mut via = board.vias.iter().find(|v| v.uuid == id)?.clone();
            let sides = [&mut via.attributes.top, &mut via.attributes.bottom];
            let mut open = false;
            for side in sides.into_iter().filter(|s| !s.tented) {
                side.expansion = grow(side.expansion);
                open = true;
            }
            open.then_some(BoardItem::Via(via))
        }
        ItemRef::Track(id) => {
            let mut track = board.segments.iter().find(|s| s.uuid == id)?.clone();
            // A track bares copper only with a manual expansion of its own.
            track.attributes.solder_mask.manual_mm()?;
            track.attributes.solder_mask = grow(track.attributes.solder_mask);
            Some(BoardItem::Segment(track))
        }
        _ => None,
    }
}
