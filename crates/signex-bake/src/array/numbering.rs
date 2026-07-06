//! Per-instance pad-numbering schemes for sketch arrays — both
//! the 1D  (Linear) and the 2D
//!  (Grid). Polar reuses the 1D form.

use std::collections::BTreeMap;

use signex_sketch::SketchError;
use signex_sketch::array::NumberingScheme;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;
use signex_sketch::solver::FullSolveOutput;

/// Resolve the pad number for the i-th instance of a linear array.
pub(super) fn derive_pad_number(
    numbering: &NumberingScheme,
    i: usize,
    params_ast: &BTreeMap<String, ExprNode>,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    match numbering {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
        } => linear_increment_number(start_expr, step_expr, i, params_ast)
            .unwrap_or_else(|| format!("{}", i)),
        NumberingScheme::BgaRowCol { .. } => {
            warnings.push(format!(
                "linear array source {source}: BgaRowCol numbering not meaningful on a 1D Linear array — falling back to LinearIncrement defaults",
                ));
            // Fall back to LinearIncrement default: 1-based, step 1.
            format!("{}", i + 1)
        }
        NumberingScheme::Explicit { names } => {
            if i < names.len() {
                names[i].clone()
            } else {
                warnings.push(format!(
                    "linear array source {source}: Explicit numbering ran out of names at i={i}; using fallback \"{i}\"",
                    ));
                format!("{}", i)
            }
        }
    }
}

/// `LinearIncrement` — `start + i * step`, both rounded to integer
/// after canonical evaluation. Returns `None` on any expression error
/// so the caller can fall back to a default scheme without aborting
/// the whole array bake.
pub(super) fn linear_increment_number(
    start_expr: &str,
    step_expr: &str,
    i: usize,
    params_ast: &BTreeMap<String, ExprNode>,
) -> Option<String> {
    let ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((i, 0)),
    };
    let start = eval(&parse(strip_eq_prefix(start_expr)).ok()?, &ctx)
        .ok()?
        .value;
    let step = eval(&parse(strip_eq_prefix(step_expr)).ok()?, &ctx)
        .ok()?
        .value;
    let n = (start + i as f64 * step).round() as i64;
    Some(format!("{}", n))
}

/// Strip the optional Altium-style leading `=` and surrounding
/// whitespace so authored expressions like `= count` parse cleanly.
pub(super) fn strip_eq_prefix(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s)
}

/// v0.22 Phase B3 — bake `ArrayKind::Grid`. Walks `(i, j)` with
/// `i in 0..nx` and `j in 0..ny`, per-instance offset
/// `(i * dx, j * dy)` from the source. Optional `depopulation` is a
/// boolean expression evaluated per cell — `false` skips the cell
/// without breaking the parametric chain.
#[allow(clippy::too_many_arguments)]
/// 2D companion to `derive_pad_number` — `BgaRowCol` is meaningful
/// here, falling back to a row-major linear count for `LinearIncrement`
/// and looking up `names[j*nx + i]` for `Explicit`.
pub(super) fn derive_pad_number_2d(
    numbering: &NumberingScheme,
    i: usize,
    j: usize,
    nx: usize,
    params_ast: &BTreeMap<String, ExprNode>,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    use signex_sketch::array::bga_row_letter;
    match numbering {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
        } => {
            // Row-major: (j, i) → idx = j*nx + i.
            let idx = j * nx + i;
            linear_increment_number(start_expr, step_expr, idx, params_ast)
                .unwrap_or_else(|| format!("{}", idx + 1))
        }
        NumberingScheme::BgaRowCol {
            skip_letters,
            start_row,
            start_col,
        } => {
            let row = bga_row_letter(j as u32, *skip_letters, *start_row);
            let col = (i as u32) + *start_col;
            format!("{row}{col}")
        }
        NumberingScheme::Explicit { names } => {
            let idx = j * nx + i;
            if idx < names.len() {
                names[idx].clone()
            } else {
                warnings.push(format!(
                    "grid array source {source}: Explicit numbering ran out of names at ({i}, {j}); using fallback \"{idx}\"",
                ));
                format!("{}", idx)
            }
        }
    }
}
