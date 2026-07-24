//! Grid (2D) array baking — `nx` × `ny` instances stepped by
//! (`dx_expr`, `dy_expr`) per axis, with optional per-cell
//! depopulation (a mask predicate and/or an explicit suppressed-cell
//! list).

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

use super::numbering::derive_pad_number_2d;
use super::numbering::strip_eq_prefix;

pub(super) fn bake_grid(
    source: SketchEntityId,
    nx_expr: &str,
    ny_expr: &str,
    dx_expr: &str,
    dy_expr: &str,
    depopulation: Option<&signex_sketch::array::GridDepopulation>,
    numbering: &NumberingScheme,
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
            warnings.push(format!(
                "grid array source {source}: no PadAttr on source entity — array skipped"
            ));
            return Ok(());
        }
    };

    let nx_ast = parse(strip_eq_prefix(nx_expr)).map_err(SketchError::Expr)?;
    let ny_ast = parse(strip_eq_prefix(ny_expr)).map_err(SketchError::Expr)?;
    let dx_ast = parse(strip_eq_prefix(dx_expr)).map_err(SketchError::Expr)?;
    let dy_ast = parse(strip_eq_prefix(dy_expr)).map_err(SketchError::Expr)?;
    // v0.23 — empty mask_expr is the "no predicate, suppression list
    // only" path. The parser rejects empty input, so skip the parse
    // when the trimmed text is empty rather than surfacing an error.
    let mask_ast = depopulation
        .filter(|d| !d.mask_expr.trim().is_empty())
        .map(|d| parse(strip_eq_prefix(&d.mask_expr)))
        .transpose()
        .map_err(SketchError::Expr)?;

    let setup_ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((0, 0)),
    };
    let nx = eval(&nx_ast, &setup_ctx)
        .map_err(SketchError::Expr)?
        .value
        .round() as i64;
    let ny = eval(&ny_ast, &setup_ctx)
        .map_err(SketchError::Expr)?
        .value
        .round() as i64;
    if nx <= 0 || ny <= 0 {
        warnings.push(format!(
            "grid array source {source}: nx={nx}, ny={ny} — array skipped"
        ));
        return Ok(());
    }
    let nx = nx as usize;
    let ny = ny as usize;

    for j in 0..ny {
        for i in 0..nx {
            let step_ctx = EvalContext {
                params: params_ast.clone(),
                array_index: Some((i, j)),
            };

            // v0.23 — Per-instance suppression list. Skipped
            // independent of the mask predicate so the Properties
            // panel checkbox grid can toggle individual cells without
            // mutating the expression.
            if let Some(d) = depopulation {
                let i_u32 = i as u32;
                let j_u32 = j as u32;
                if d.suppressed_instances
                    .iter()
                    .any(|(si, sj)| *si == i_u32 && *sj == j_u32)
                {
                    continue;
                }
            }

            // Depopulation predicate — non-zero / true keeps the
            // cell, zero / false skips. Defaults to keep on parse
            // / eval failure (warned).
            if let Some(ast) = mask_ast.as_ref() {
                match eval(ast, &step_ctx) {
                    Ok(q) => {
                        if q.value.abs() < 1e-9 {
                            continue;
                        }
                    }
                    Err(e) => warnings.push(format!(
                        "grid array source {source}: depopulation eval failed at ({i}, {j}): {e:?} — keeping cell"
                    )),
                }
            }

            let dx_q = eval(&dx_ast, &step_ctx).map_err(SketchError::Expr)?;
            let dy_q = eval(&dy_ast, &step_ctx).map_err(SketchError::Expr)?;
            let dx_mm = dx_q.as_mm().map_err(SketchError::Unit)?;
            let dy_mm = dy_q.as_mm().map_err(SketchError::Unit)?;

            let extra_dx = i as f64 * dx_mm;
            let extra_dy = j as f64 * dy_mm;

            let pad_number =
                derive_pad_number_2d(numbering, i, j, nx, params_ast, warnings, source);

            let pad = bake_one_pad(
                source,
                pad_attr,
                params_ast,
                Some((i, j)),
                extra_dx,
                extra_dy,
                Some(pad_number),
                sketch,
                solve,
                warnings,
            )?;
            out.push(pad);
        }
    }

    Ok(())
}
