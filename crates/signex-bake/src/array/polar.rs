//! Polar (rotational) array baking — `count` instances around a
//! centre point, sweeping `sweep_angle_expr` total degrees, with
//! optional per-instance depopulation.

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

use super::numbering::derive_pad_number;
use super::numbering::strip_eq_prefix;

/// v0.22 Phase B4 — bake `ArrayKind::Polar`. Walks `i in 0..count`,
/// per-instance position is the source rotated by
/// `i * sweep_angle_rad / count` around `center` (the source itself
/// stays in place at i=0 — matches Altium's polar-array convention).
///
/// v0.22 Phase B5 — optional `depopulation` works the same as Grid:
/// `mask_expr` is evaluated per `(i, j=0)` and `false` skips the
/// instance without breaking the parametric chain.
#[allow(clippy::too_many_arguments)]
pub(super) fn bake_polar(
    source: SketchEntityId,
    center: SketchEntityId,
    count_expr: &str,
    sweep_angle_expr: &str,
    depopulation: Option<&signex_sketch::array::GridDepopulation>,
    numbering: &NumberingScheme,
    params_ast: &BTreeMap<String, ExprNode>,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    use signex_sketch::solver::state::point_xy;

    let source_entity = match sketch.entities.iter().find(|e| e.id == source) {
        Some(e) => e,
        None => {
            warnings.push(format!(
                "polar array source {source}: entity not found — array skipped"
            ));
            return Ok(());
        }
    };
    let pad_attr = match source_entity.pad.as_ref() {
        Some(p) => p,
        None => {
            warnings.push(format!(
                "polar array source {source}: no PadAttr on source entity — array skipped"
            ));
            return Ok(());
        }
    };

    // Resolve source + center positions in mm.
    let (sx, sy) = match point_xy(source, &solve.result.state, &solve.result.index, sketch) {
        Some(p) => p,
        None => {
            warnings.push(format!(
                "polar array source {source}: position unknown — array skipped"
            ));
            return Ok(());
        }
    };
    let (cx, cy) = match point_xy(center, &solve.result.state, &solve.result.index, sketch) {
        Some(p) => p,
        None => {
            warnings.push(format!(
                "polar array center {center}: position unknown — array skipped"
            ));
            return Ok(());
        }
    };

    let count_ast = parse(strip_eq_prefix(count_expr)).map_err(SketchError::Expr)?;
    let sweep_ast = parse(strip_eq_prefix(sweep_angle_expr)).map_err(SketchError::Expr)?;
    let setup_ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((0, 0)),
    };
    let count = eval(&count_ast, &setup_ctx)
        .map_err(SketchError::Expr)?
        .value
        .round() as i64;
    if count <= 0 {
        warnings.push(format!(
            "polar array source {source}: count={count} — array skipped"
        ));
        return Ok(());
    }
    let count = count as usize;

    // Sweep angle is an Altium-style degrees-or-radians value via the
    // expression's unit family. We expect rad here (the parser
    // resolves `deg` → rad). On unit error, surface and skip.
    let sweep_q = eval(&sweep_ast, &setup_ctx).map_err(SketchError::Expr)?;
    let sweep_rad = match sweep_q.unit.family() {
        signex_sketch::unit::UnitFamily::Angle => sweep_q.value,
        _ => {
            warnings.push(format!(
                "polar array source {source}: sweep_angle_expr did not resolve to an angle — array skipped"
            ));
            return Ok(());
        }
    };

    let denom = (count as f64).max(1.0);
    let dx_src = sx - cx;
    let dy_src = sy - cy;

    // v0.22 Phase B5 — pre-parse the depopulation mask, if any.
    // v0.23 — empty mask_expr is the "suppression list only" path
    // (the parser rejects empty input).
    let mask_ast = depopulation
        .filter(|d| !d.mask_expr.trim().is_empty())
        .map(|d| parse(strip_eq_prefix(&d.mask_expr)))
        .transpose()
        .map_err(SketchError::Expr)?;

    for i in 0..count {
        // v0.23 — Per-instance suppression list (`j` is always 0 for
        // Polar). Skipped independent of the mask predicate so the
        // Properties panel checkbox row can toggle individual
        // instances without rewriting the expression.
        if let Some(d) = depopulation {
            let i_u32 = i as u32;
            if d.suppressed_instances
                .iter()
                .any(|(si, sj)| *si == i_u32 && *sj == 0)
            {
                continue;
            }
        }

        // Depopulation predicate — non-zero / true keeps the
        // instance, zero / false skips. `j` is bound to 0 (Grid's
        // second axis isn't meaningful for Polar). Defaults to keep
        // on eval failure (warned, never silently dropped).
        if let Some(ast) = mask_ast.as_ref() {
            let mask_ctx = EvalContext {
                params: params_ast.clone(),
                array_index: Some((i, 0)),
            };
            match eval(ast, &mask_ctx) {
                Ok(q) => {
                    if q.value.abs() < 1e-9 {
                        continue;
                    }
                }
                Err(e) => warnings.push(format!(
                    "polar array source {source}: depopulation eval failed at i={i}: {e:?} — keeping instance"
                )),
            }
        }

        let theta = (i as f64) * sweep_rad / denom;
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let rx = cx + dx_src * cos_t - dy_src * sin_t;
        let ry = cy + dx_src * sin_t + dy_src * cos_t;
        // Position offset = rotated_source - source. bake_one_pad
        // adds this on top of the source's own position so the i=0
        // case matches the source exactly.
        let extra_dx = rx - sx;
        let extra_dy = ry - sy;

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
