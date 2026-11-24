//! The min slot check of a board sketch (mockup row 179d): every Slot
//! opening is at least as wide as the fab's min slot. It reads the slot's
//! width as the bake does (`slot::check`), so it sees what Bake routes; a
//! Slot profile that is not a slot is the bake's to refuse. A pattern
//! repeats one width, so the source names every instance. Warns only, like
//! every outline check. Rules: `docs/PCB_SKETCH.md`, "Outline checks".

use signex_sketch::baked_kinds::BakedKind;

use super::checks::Finding;
use super::features::is_slot;
use super::geometry::boundary;
use super::{Context, slot};
use crate::traced_loops::TracedLoops;

/// How far, in mm, a slot may be under the min slot and still pass: a
/// slot drawn at exactly the min slot solves a hair away from it.
const WIDTH_TOLERANCE_MM: f64 = 1e-6;

/// Each Slot narrower than `min_slot` mm, once per loop, named by the
/// entity its cutout takes its id from.
pub(super) fn narrow_slots(ctx: &Context<'_>, min_slot: f64) -> Vec<Finding> {
    let mut loops = TracedLoops::default();
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.board_cutout.is_some()) {
        let attribute = format!("{:?}", entity.board_cutout);
        // The walk records the loop, so its other edges are not seen again.
        let traced = boundary(ctx, &mut loops, &attribute, entity, BakedKind::Cutouts);
        if !matches!(traced, Ok(Some(_))) || !is_slot(ctx, entity) {
            continue;
        }
        let Ok(slot) = slot::check(ctx, entity.id) else {
            continue;
        };
        if slot.width + WIDTH_TOLERANCE_MM < min_slot {
            out.push(Finding::NarrowSlot {
                entity: entity.id,
                width_mm: slot.width,
                min_slot_mm: min_slot,
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "min_slot_tests.rs"]
mod tests;
