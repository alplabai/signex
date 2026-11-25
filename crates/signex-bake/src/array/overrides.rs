//! Baking the instances of a linked pattern or mirror, each with its own
//! per-instance pad override (mockup row 192, item D1) applied to the source pad before the
//! pad bake, so only that instance changes.

use std::borrow::Cow;
use std::collections::BTreeMap;

use signex_expr::ExprNode;
use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::attr::PadAttr;
use signex_sketch::expr::eval::EvalContext;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use super::links::{Basis, Expansion, Instance};
use super::numbering::{derive_pad_number, derive_swept_pad_number};
use crate::pad::bake_one_pad;

/// The pad an instance bakes from: `source` with the instance's override,
/// if it has one, written over it.
pub(crate) fn attr_for<'p>(
    sketch: &SketchData,
    instance: &Instance,
    source: &'p PadAttr,
) -> Cow<'p, PadAttr> {
    let (array, i, j) = instance.key;
    match sketch.override_for(array, i, j) {
        Some(o) => {
            let mut attr = source.clone();
            o.apply_to(&mut attr);
            Cow::Owned(attr)
        }
        None => Cow::Borrowed(source),
    }
}

/// The number of the instance at `flat` in `basis`.
fn number_at(
    basis: &Basis<'_>,
    flat: u64,
    ctx: &mut EvalContext,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    let flat = usize::try_from(flat).unwrap_or(usize::MAX);
    match basis.nx {
        Some(nx) if nx > 0 => derive_swept_pad_number(
            basis.numbering,
            flat % nx,
            flat / nx,
            nx,
            basis.ny,
            ctx,
            warnings,
            source,
        ),
        _ => derive_pad_number(basis.numbering, flat, ctx, warnings, source),
    }
}

/// Bake every instance `expansion` places itself (its `own` ones) from the
/// pad on `source`, into `out`.
pub(crate) fn bake_instances(
    source: SketchEntityId,
    expansion: &Expansion<'_>,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_ast: &BTreeMap<String, ExprNode>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let Some(pad) = sketch
        .entities
        .iter()
        .find(|e| e.id == source)
        .and_then(|e| e.pad.as_ref())
    else {
        warnings.push(super::not_a_pad("pattern", sketch, solve, source));
        return Ok(());
    };
    let Some(at) = point_xy(source, &solve.result.state, &solve.result.index, sketch) else {
        warnings.push(format!(
            "pattern source {source}: position unknown — pattern skipped"
        ));
        return Ok(());
    };
    let mut ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((0, 0)),
    };
    for instance in expansion.instances.iter().filter(|x| x.own) {
        let attr = attr_for(sketch, instance, pad);
        let number = match sketch
            .override_for(instance.key.0, instance.key.1, instance.key.2)
            .and_then(|o| o.number.clone())
        {
            Some(number) => number,
            None => number_at(&expansion.basis, instance.flat, &mut ctx, warnings, source),
        };
        ctx.array_index = Some(instance.index);
        let (x, y) = instance.place.apply(at);
        let mut baked = bake_one_pad(
            source,
            &attr,
            &ctx,
            x - at.0,
            y - at.1,
            Some(number),
            sketch,
            solve,
            warnings,
        )?;
        baked.rotation = instance.place.turn_deg(baked.rotation);
        out.push(baked);
    }
    Ok(())
}
