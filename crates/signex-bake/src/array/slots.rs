//! The instance slots of one array as its bake lays them out, for the
//! sketch canvas's instance rings: the same counts, per-instance steps,
//! sweep and mask as the bake (one walk, [`super::links::Walk`]), so a ring
//! sits where the bake puts the pad.

use signex_sketch::array::ArrayId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use super::links::Walk;

/// One instance slot of an array.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatternSlot {
    /// Index along the array and, for a grid, the row.
    pub i: u32,
    pub j: u32,
    /// Where the slot places the point the source sits at, mm.
    pub at: (f64, f64),
    /// In the suppression list: the bake leaves it out.
    pub suppressed: bool,
}

/// Every slot of array `id` the bake places or its suppression list holds
/// back, the source's `(0, 0)` first, each placing `source_at` (the
/// source's anchor, mm). A slot the depopulation mask drops is left out:
/// the bake places nothing there and the list cannot bring it back.
/// `None` when the array is missing, does not expand (a count, step or
/// sweep that does not evaluate), or has more than `limit` slots.
pub fn pattern_slots(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    id: ArrayId,
    source_at: (f64, f64),
    limit: u64,
) -> Option<Vec<PatternSlot>> {
    let array = sketch.arrays.iter().find(|a| a.id == id)?;
    let resolved = signex_sketch::parameter::resolve(&sketch.parameters).ok()?;
    let params_ast = crate::params::params_ast(sketch, &resolved);
    let mut ignored = Vec::new();
    let walked = Walk::new(sketch, solve, &params_ast, &mut ignored)
        .keeping_listed()
        .walk_slots(array)
        .ok()??;
    if walked.count > limit {
        return None;
    }
    let slots = walked
        .slots
        .iter()
        .filter_map(|s| {
            Some(PatternSlot {
                i: u32::try_from(s.i).ok()?,
                j: u32::try_from(s.j).ok()?,
                at: s.place.apply(source_at),
                suppressed: s.listed,
            })
        })
        .collect();
    Some(slots)
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
