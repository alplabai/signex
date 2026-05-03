//! Sketch-array expansion — v0.13 bakes ArrayKind::Linear only.
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
//! # v0.13 scope
//!
//! - `ArrayKind::Linear { source, count_expr, dx_expr, dy_expr }` —
//!   bakes natively.
//! - `ArrayKind::Grid { .. }` — emits a single warning, no bake.
//! - `ArrayKind::Polar { .. }` — emits a single warning, no bake.
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
use signex_sketch::array::{ArrayKind, NumberingScheme};
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{eval, EvalContext};
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::unit::Quantity;
use signex_sketch::SketchError;

use crate::pad::bake_one_pad;

/// Walk every [`signex_sketch::array::Array`] and append baked pads
/// to `out`. v0.13 bakes `ArrayKind::Linear` only; Grid + Polar emit
/// a single warning each and produce no pads.
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
            ArrayKind::Grid { .. } => warnings.push(format!(
                "array {}: ArrayKind::Grid bake deferred to v0.14",
                array.id.0
            )),
            ArrayKind::Polar { .. } => warnings.push(format!(
                "array {}: ArrayKind::Polar bake deferred to v0.14",
                array.id.0
            )),
        }
    }

    Ok(())
}

/// One LinearArray expansion. Steps `0..count`, evaluating `dx_expr`
/// / `dy_expr` once per step (each in its own `EvalContext` with the
/// instance index), and dispatches per-instance to
/// [`bake_one_pad`].
#[allow(clippy::too_many_arguments)]
fn bake_linear(
    source: SketchEntityId,
    count_expr: &str,
    dx_expr: &str,
    dy_expr: &str,
    numbering: &NumberingScheme,
    params_ast: &BTreeMap<String, ExprNode>,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    // Find the source entity's PadAttr. Without it there's nothing to
    // replicate; warn and skip the array.
    let source_entity = match sketch.entities.iter().find(|e| e.id == source) {
        Some(e) => e,
        None => {
            warnings.push(format!(
                "linear array source {source}: entity not found — array skipped"
            ));
            return Ok(());
        }
    };
    let pad_attr = match source_entity.pad.as_ref() {
        Some(p) => p,
        None => {
            warnings.push(format!(
                "linear array source {source}: no PadAttr on source entity — array skipped"
            ));
            return Ok(());
        }
    };

    // Evaluate count once with array_index = (0, 0) — the count must
    // be a constant or parameter; the spec doesn't carry per-step
    // counts.
    let setup_ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((0, 0)),
    };
    let count_q = eval(
        &parse(strip_eq_prefix(count_expr)).map_err(SketchError::Expr)?,
        &setup_ctx,
    )
    .map_err(SketchError::Expr)?;
    let count = count_q.value.round() as i64;
    if count <= 0 {
        warnings.push(format!(
            "linear array source {source}: count_expr resolved to {count} — array skipped"
        ));
        return Ok(());
    }
    let count = count as usize;

    for i in 0..count {
        let step_ctx = EvalContext {
            params: params_ast.clone(),
            array_index: Some((i, 0)),
        };

        // dx / dy are length expressions in mm.
        let dx_ast = parse(strip_eq_prefix(dx_expr)).map_err(SketchError::Expr)?;
        let dy_ast = parse(strip_eq_prefix(dy_expr)).map_err(SketchError::Expr)?;
        let dx_q = eval(&dx_ast, &step_ctx).map_err(SketchError::Expr)?;
        let dy_q = eval(&dy_ast, &step_ctx).map_err(SketchError::Expr)?;
        let dx_mm = dx_q.as_mm().map_err(SketchError::Unit)?;
        let dy_mm = dy_q.as_mm().map_err(SketchError::Unit)?;

        let extra_dx = i as f64 * dx_mm;
        let extra_dy = i as f64 * dy_mm;

        let pad_number = derive_pad_number(numbering, i, params_ast, warnings, source);

        let pad = bake_one_pad(
            source,
            pad_attr,
            params_ast,
            Some((i, 0)),
            extra_dx,
            extra_dy,
            Some(pad_number),
            sketch,
            solve,
            warnings,
        )?;
        out.push(pad);
    }

    Ok(())
}

/// Resolve the pad number for the i-th instance of a linear array.
fn derive_pad_number(
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
fn linear_increment_number(
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
fn strip_eq_prefix(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s)
}
