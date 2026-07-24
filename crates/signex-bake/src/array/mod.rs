//! Sketch-array expansion — bakes all three `ArrayKind` variants.
//!
//! Phase 7 Task 7.2 of the v0.13 sketch-mode plan. Walks every
//! `Array` in the sketch and produces baked
//! [`signex_library::primitive::footprint::Pad`]s by re-using the
//! per-pad bake body from `crate::pad::bake_one_pad`.
//!
//! Cleanroom: derived from first principles + the Phase 4 expression
//! machinery. No third-party constraint-solver, footprint-generator,
//! or numerical-library source consulted.
//!
//! # Scope
//!
//! - `ArrayKind::Linear { source, count_expr, dx_expr, dy_expr }` —
//!   bakes natively (`array::linear`).
//! - `ArrayKind::Grid { .. }` — bakes natively (`array::grid`).
//! - `ArrayKind::Polar { .. }` — bakes natively (`array::polar`).
//!
//! Numbering:
//! - `LinearIncrement { start, step }` — pad number =
//!   `start + i * step` rounded to integer.
//! - `BgaRowCol { .. }` on a 1D Linear — warns (BGA on Linear is not
//!   semantically meaningful) and falls back to a default 1-based
//!   `LinearIncrement`.
//! - `Explicit { names }` — uses `names[i]` if present; warns and
//!   falls back to `format!("{i}")` otherwise.

use std::collections::{BTreeMap, HashMap};

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::{ArrayKind, NumberingScheme};
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::unit::Quantity;

use crate::pad::bake_one_pad;

mod grid;
mod linear;
mod numbering;
mod polar;

use grid::bake_grid;
use linear::bake_linear;
use polar::bake_polar;

/// Walk every [`signex_sketch::array::Array`] and append baked pads
/// to `out`. Bakes `ArrayKind::Linear`, `Grid`, and `Polar` natively.
pub fn bake_arrays(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let mut params_ast: BTreeMap<String, ExprNode> = BTreeMap::new();
    for (name, value) in params_canonical {
        params_ast.insert(name.clone(), ExprNode::Literal(Quantity::length(*value)));
    }

    for array in &sketch.arrays {
        match &array.kind {
            ArrayKind::Linear {
                source,
                count_expr,
                dx_expr,
                dy_expr,
            } => {
                bake_linear(
                    *source,
                    count_expr,
                    dx_expr,
                    dy_expr,
                    &array.numbering,
                    &params_ast,
                    sketch,
                    solve,
                    out,
                    warnings,
                )?;
            }
            ArrayKind::Grid {
                source,
                nx_expr,
                ny_expr,
                dx_expr,
                dy_expr,
                depopulation,
            } => {
                bake_grid(
                    *source,
                    nx_expr,
                    ny_expr,
                    dx_expr,
                    dy_expr,
                    depopulation.as_ref(),
                    &array.numbering,
                    &params_ast,
                    sketch,
                    solve,
                    out,
                    warnings,
                )?;
            }
            ArrayKind::Polar {
                source,
                center,
                count_expr,
                sweep_angle_expr,
                depopulation,
            } => {
                bake_polar(
                    *source,
                    *center,
                    count_expr,
                    sweep_angle_expr,
                    depopulation.as_ref(),
                    &array.numbering,
                    &params_ast,
                    sketch,
                    solve,
                    out,
                    warnings,
                )?;
            }
        }
    }

    Ok(())
}
