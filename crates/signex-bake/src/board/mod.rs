//! Board sketch → board geometry (#2234): the outline, keepouts, cutouts,
//! mounting holes and V-scores of a board, from the same parametric sketch
//! the footprint editor uses.
//!
//! [`bake_board`] is pure: it resolves the parameters, solves the sketch and
//! reads the solved positions, and touches nothing else. The caller stores
//! the result with `BoardSketch::from_bake` and commits it as one board
//! command. Construction and centerline entities never bake
//! (`Entity::bake_skipped`). Rules and formats: `docs/PCB_SKETCH.md`.

mod error;
mod features;
mod geometry;

#[cfg(test)]
mod tests;

pub use error::BoardBakeError;

use std::collections::HashMap;

use signex_sketch::Solver;
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::Entity;
use signex_sketch::parameter::resolve;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::pcb::{NetDef, Point};
use signex_types::pcb_sketch::BakedBoard;

use crate::traced_loops::TracedLoops;
use geometry::boundary;

/// Bake `sketch` into board geometry. `nets` is the board's net table: a
/// mounting hole names its net, and the hole stores that net's number.
pub fn bake_board(sketch: &SketchData, nets: &[NetDef]) -> Result<BakedBoard, BoardBakeError> {
    let params = resolve(&sketch.parameters).map_err(BoardBakeError::Parameters)?;
    let solve = Solver::default()
        .solve(sketch, &params)
        .map_err(BoardBakeError::Solve)?;
    let ctx = Context {
        sketch,
        solve: &solve,
        params: &params,
    };
    Ok(BakedBoard {
        outline: outline(&ctx)?,
        keepouts: features::keepouts(&ctx)?,
        cutouts: features::cutouts(&ctx)?,
        holes: features::holes(&ctx, nets)?,
        v_scores: features::v_scores(&ctx)?,
    })
}

/// What every bake stage reads.
pub(crate) struct Context<'a> {
    pub(crate) sketch: &'a SketchData,
    pub(crate) solve: &'a FullSolveOutput,
    pub(crate) params: &'a HashMap<String, f64>,
}

impl Context<'_> {
    /// Entities that bake (not construction or centerline) and pass `tagged`.
    pub(crate) fn tagged<'s>(
        &'s self,
        tagged: impl Fn(&Entity) -> bool + 's,
    ) -> impl Iterator<Item = &'s Entity> + 's {
        self.sketch
            .entities
            .iter()
            .filter(move |e| !e.bake_skipped() && tagged(e))
    }
}

/// The outline: exactly one closed profile of the edges tagged
/// `board_outline` (a loop of lines and arcs, or one circle). With none
/// tagged, an empty outline if the sketch baked one before (it was
/// deleted), else `None`: the board keeps its own.
fn outline(ctx: &Context<'_>) -> Result<Option<Vec<Point>>, BoardBakeError> {
    const KIND: BakedKind = BakedKind::BoardOutline;
    let mut loops = TracedLoops::default();
    let mut found: Vec<Vec<Point>> = Vec::new();
    for entity in ctx.tagged(|e| e.board_outline) {
        if let Some(profile) = boundary(ctx, &mut loops, "outline", entity, KIND)? {
            found.push(profile);
        }
    }
    match found.len() {
        0 => Ok(ctx.sketch.was_baked(KIND).then(Vec::new)),
        1 => Ok(found.pop()),
        count => Err(BoardBakeError::OutlineCount { count }),
    }
}
