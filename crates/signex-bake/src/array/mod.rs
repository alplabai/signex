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

use std::collections::HashMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::ArrayKind;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

mod grid;
mod linear;
mod numbering;
mod polar;

/// Most pad instances one array may expand to. Array counts come from
/// user / file expressions and the bake runs on the editor's update
/// thread, so an unbounded count (`=100000` by `=100000`) would freeze
/// and then exhaust memory. Real packages (a 40x40 BGA) sit far below it.
pub const MAX_ARRAY_INSTANCES: usize = 10_000;

/// Whether an array expanding to `instances` pads (`None` when the count
/// overflowed `usize`) stays within [`MAX_ARRAY_INSTANCES`]. Pushes a bake
/// warning and returns `false` when it does not, so the caller skips the
/// array.
fn within_instance_cap(
    label: &str,
    source: signex_sketch::id::SketchEntityId,
    instances: Option<usize>,
    warnings: &mut Vec<String>,
) -> bool {
    match instances {
        Some(n) if n <= MAX_ARRAY_INSTANCES => true,
        Some(n) => {
            warnings.push(format!(
                "{label} array source {source}: {n} instances exceeds the limit of \
                 {MAX_ARRAY_INSTANCES} — array skipped"
            ));
            false
        }
        None => {
            warnings.push(format!(
                "{label} array source {source}: instance count overflows, limit is \
                 {MAX_ARRAY_INSTANCES} instances — array skipped"
            ));
            false
        }
    }
}

use grid::{GridSpec, bake_grid};
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
    let params_ast = crate::params::params_ast(sketch, params_canonical);

    for array in &sketch.arrays {
        bake_one_array(array, &params_ast, sketch, solve, out, warnings)?;
    }

    Ok(())
}

/// Bake one array's instances into `out`.
pub(crate) fn bake_one_array(
    array: &signex_sketch::array::Array,
    params_ast: &std::collections::BTreeMap<String, signex_sketch::expr::ast::ExprNode>,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
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
                params_ast,
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
            let spec = GridSpec {
                nx_expr,
                ny_expr,
                dx_expr,
                dy_expr,
                depopulation: depopulation.as_ref(),
                numbering: &array.numbering,
            };
            bake_grid(*source, &spec, params_ast, sketch, solve, out, warnings)?;
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
                params_ast,
                sketch,
                solve,
                out,
                warnings,
            )?;
        }
    }
    Ok(())
}
