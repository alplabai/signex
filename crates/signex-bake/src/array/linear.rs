//! Linear array baking —  instances stepped by
//!  from the source pad.

use std::collections::BTreeMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::NumberingScheme;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::EvalContext;
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use crate::pad::bake_one_pad;

use super::numbering::derive_pad_number;
use crate::eval_util::strip_eq_prefix;

/// One LinearArray expansion. Steps `0..count`, evaluating `dx_expr`
/// / `dy_expr` once per step (each in its own `EvalContext` with the
/// instance index), and dispatches per-instance to
/// [`bake_one_pad`].
#[expect(
    clippy::too_many_arguments,
    reason = "linear-array placement takes the whole geometry + numbering spec at once"
)]
pub(super) fn bake_linear(
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
    let mut ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((0, 0)),
    };
    let Some(count) =
        super::eval_count("linear", "count_expr", source, &count_ast, &ctx, warnings)?
    else {
        return Ok(());
    };
    if count <= 0 {
        warnings.push(format!(
            "linear array source {source}: count_expr resolved to {count} — array skipped"
        ));
        return Ok(());
    }
    let count = count as usize;
    if !super::within_instance_cap("linear", source, Some(count), warnings) {
        return Ok(());
    }

    for i in 0..count {
        // One context for the whole array: only `array_index` changes
        // between instances, so the parameter table is cloned once.
        ctx.array_index = Some((i, 0));

        // dx / dy are length expressions in mm.
        let dx_mm = crate::params::eval_length_mm(&dx_ast, &ctx)?;
        let dy_mm = crate::params::eval_length_mm(&dy_ast, &ctx)?;

        let extra_dx = i as f64 * dx_mm;
        let extra_dy = i as f64 * dy_mm;

        let pad_number = derive_pad_number(numbering, i, &mut ctx, warnings, source);
        ctx.array_index = Some((i, 0));

        let pad = bake_one_pad(
            source,
            pad_attr,
            &ctx,
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
