//! Grid (2D) array baking — `nx` × `ny` instances stepped by
//! (`dx_expr`, `dy_expr`) per axis, with optional per-cell
//! depopulation (a mask predicate and/or an explicit suppressed-cell
//! list).

use std::collections::BTreeMap;

use signex_expr::ExprNode;
use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::NumberingScheme;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use super::walk::{Walk, Walked};

/// The grid array's declarative shape: the `(nx, ny)` count expressions
/// stepped by `(dx, dy)` per axis, its depopulation predicate/list, and
/// its numbering scheme. Built once per `ArrayKind::Grid` in
/// `array::bake_arrays` and passed by reference into [`bake_grid`] —
/// none of these fields are mutated during baking.
pub(super) struct GridSpec<'a> {
    pub nx_expr: &'a str,
    pub ny_expr: &'a str,
    pub dx_expr: &'a str,
    pub dy_expr: &'a str,
    pub depopulation: Option<&'a signex_sketch::array::GridDepopulation>,
    pub numbering: &'a NumberingScheme,
}

/// Walks the cells ([`Walk::grid`]) and bakes every kept one through
/// [`super::bake_instance`]. A dropped cell (suppressed list, or a mask
/// that evaluates to zero) is never baked; a mask that fails to evaluate
/// keeps the cell, with a warning.
pub(super) fn bake_grid(
    source: SketchEntityId,
    spec: &GridSpec,
    params_ast: &BTreeMap<String, ExprNode>,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let source_entity = match sketch.entities.iter().find(|e| e.id == source) {
        Some(e) => e,
        None => {
            warnings.push(format!(
                "grid array source {source}: entity not found — array skipped"
            ));
            return Ok(());
        }
    };
    let pad_attr = match source_entity.pad.as_ref() {
        Some(p) => p,
        None => {
            warnings.push(super::not_a_pad("grid array", sketch, solve, source));
            return Ok(());
        }
    };

    let walk = Walk {
        source,
        numbering: spec.numbering,
        params_ast,
        limit: super::MAX_ARRAY_INSTANCES,
        visit_skipped: false,
    };
    let walked = walk.grid(
        spec.nx_expr,
        spec.ny_expr,
        spec.dx_expr,
        spec.dy_expr,
        spec.depopulation,
        warnings,
        &mut |instance, ctx, warnings| {
            super::bake_instance(
                source, pad_attr, instance, ctx, sketch, solve, out, warnings,
            )
        },
    )?;
    if let Walked::TooMany(n) = walked {
        super::within_instance_cap("grid", source, n, warnings);
    }
    Ok(())
}
