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
            } => {
                bake_polar(
                    *source,
                    *center,
                    count_expr,
                    sweep_angle_expr,
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

    // MD-1: parse the dx/dy/count expressions ONCE outside the loop —
    // the AST never changes between instances, only the `array_index`
    // bound in the EvalContext does. For a 200-pin BGA this drops
    // 400+ redundant `parse()` calls.
    let count_ast = parse(strip_eq_prefix(count_expr)).map_err(SketchError::Expr)?;
    let dx_ast = parse(strip_eq_prefix(dx_expr)).map_err(SketchError::Expr)?;
    let dy_ast = parse(strip_eq_prefix(dy_expr)).map_err(SketchError::Expr)?;

    // Evaluate count once with array_index = (0, 0) — the count must
    // be a constant or parameter; the spec doesn't carry per-step
    // counts.
    let setup_ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((0, 0)),
    };
    let count_q = eval(&count_ast, &setup_ctx).map_err(SketchError::Expr)?;
    let count = count_q.value.round() as i64;
    if count <= 0 {
        warnings.push(format!(
            "linear array source {source}: count_expr resolved to {count} — array skipped"
        ));
        return Ok(());
    }
    let count = count as usize;

    for i in 0..count {
        // MD-2: re-evaluating dx/dy needs only the `array_index` change,
        // but `EvalContext.params` is owned by value. Cloning the full
        // BTreeMap per step is `O(count × params)` work. Build the
        // cloned params once and only swap the index per iteration.
        let step_ctx = EvalContext {
            params: params_ast.clone(),
            array_index: Some((i, 0)),
        };

        // dx / dy are length expressions in mm.
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

/// v0.22 Phase B3 — bake `ArrayKind::Grid`. Walks `(i, j)` with
/// `i in 0..nx` and `j in 0..ny`, per-instance offset
/// `(i * dx, j * dy)` from the source. Optional `depopulation` is a
/// boolean expression evaluated per cell — `false` skips the cell
/// without breaking the parametric chain.
#[allow(clippy::too_many_arguments)]
fn bake_grid(
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
    let mask_ast = depopulation
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

/// v0.22 Phase B4 — bake `ArrayKind::Polar`. Walks `i in 0..count`,
/// per-instance position is the source rotated by
/// `i * sweep_angle_rad / count` around `center` (the source itself
/// stays in place at i=0 — matches Altium's polar-array convention).
#[allow(clippy::too_many_arguments)]
fn bake_polar(
    source: SketchEntityId,
    center: SketchEntityId,
    count_expr: &str,
    sweep_angle_expr: &str,
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

    for i in 0..count {
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

/// 2D companion to `derive_pad_number` — `BgaRowCol` is meaningful
/// here, falling back to a row-major linear count for `LinearIncrement`
/// and looking up `names[j*nx + i]` for `Explicit`.
fn derive_pad_number_2d(
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
