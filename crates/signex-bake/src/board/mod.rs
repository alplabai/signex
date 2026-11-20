//! Board sketch → board geometry (#2234): the outline, keepouts, cutouts,
//! mounting holes and V-scores of a board, from the same parametric sketch
//! the footprint editor uses.
//!
//! [`bake_board`] is pure: it resolves the parameters, solves the sketch and
//! reads the solved positions, and touches nothing else. The caller stores
//! the result with `BoardSketch::from_bake` and commits it as one board
//! command. Construction and centerline entities never bake
//! (`Entity::bake_skipped`). Rules and formats: `docs/PCB_SKETCH.md`.

mod checks;
mod error;
mod features;
mod geometry;
mod items;
mod slot;

#[cfg(test)]
mod tests;

pub use checks::{
    CheckState, Finding, NotChecked, OutlineCheck, OutlineChecks, outline_checks,
    outline_checks_after,
};
pub use error::BoardBakeError;
pub use items::{bake_board_items, bake_board_with_items, baked_item_count};
pub use slot::{Slot, slot_size};

use std::collections::HashMap;

use signex_sketch::Solver;
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::Entity;
use signex_sketch::id::SketchEntityId;
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
    solved(sketch, |ctx| baked(ctx, nets))
}

/// Resolve and solve `sketch` once and run `bake` on the result.
pub(crate) fn solved<T>(
    sketch: &SketchData,
    bake: impl FnOnce(&Context<'_>) -> Result<T, BoardBakeError>,
) -> Result<T, BoardBakeError> {
    let params = resolve(&sketch.parameters).map_err(BoardBakeError::Parameters)?;
    let solve = Solver::default()
        .solve(sketch, &params)
        .map_err(BoardBakeError::Solve)?;
    bake(&Context {
        sketch,
        solve: &solve,
        params: &params,
    })
}

fn baked(ctx: &Context<'_>, nets: &[NetDef]) -> Result<BakedBoard, BoardBakeError> {
    Ok(BakedBoard {
        outline: outline(ctx)?,
        keepouts: features::keepouts(ctx)?,
        cutouts: features::cutouts(ctx)?,
        holes: features::holes(ctx, nets)?,
        v_scores: features::v_scores(ctx)?,
    })
}

/// How many items of `kind` a board bake produced, for the bake preview
/// (#2518). The outline counts one when the sketch draws it. `None`: a
/// board bake never writes `kind`; the targets it writes are
/// `SketchHost::Board.bake_targets()`.
pub fn baked_count(baked: &BakedBoard, kind: BakedKind) -> Option<usize> {
    Some(match kind {
        BakedKind::BoardOutline => baked
            .outline
            .as_ref()
            .is_some_and(|outline| !outline.is_empty())
            .into(),
        BakedKind::Keepouts => baked.keepouts.len(),
        BakedKind::Cutouts => baked.cutouts.len(),
        BakedKind::MountingHoles => baked.holes.len(),
        BakedKind::VScores => baked.v_scores.len(),
        _ => return None,
    })
}

impl BoardBakeError {
    /// The sketch entity the refusal is about, for the bake preview to
    /// select; `None` when it is about the whole sketch.
    pub fn place(&self) -> Option<SketchEntityId> {
        match *self {
            Self::OpenProfile { edge, .. }
            | Self::BranchingProfile { edge, .. }
            | Self::UntraceableProfile { edge, .. } => Some(edge),
            Self::NotAnEdge { entity, .. }
            | Self::HoleNotACircle { entity }
            | Self::HoleDrill { entity, .. }
            | Self::UnknownNet { entity, .. }
            | Self::NotASlot { entity }
            | Self::PourNotCopper { entity }
            | Self::AnchorNotAPoint { entity }
            | Self::AnchorWithoutComponent { entity }
            | Self::UnknownComponent { entity, .. }
            | Self::VScoreNotALine { entity }
            | Self::Expression { entity, .. } => Some(entity),
            Self::MissingPoint { point, .. } => Some(point),
            Self::Parameters(_) | Self::Solve(_) | Self::OutlineCount { .. } => None,
        }
    }
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
