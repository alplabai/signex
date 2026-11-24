//! Polar (rotational) array baking — `count` instances around a
//! centre point, sweeping `sweep_angle_expr` total degrees, with
//! optional per-instance depopulation.

use std::collections::BTreeMap;

use signex_expr::ExprNode;
use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::NumberingScheme;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use super::walk::{Walk, Walked};

/// v0.22 Phase B4 — bake `ArrayKind::Polar`. Walks `i in 0..count`
/// ([`Walk::polar`]): instance `i` is the source turned by
/// `i * sweep_angle_rad / count` around `center`, so the source itself
/// stays in place at i = 0.
///
/// v0.22 Phase B5 — optional `depopulation` works the same as Grid:
/// `mask_expr` is evaluated per `(i, j=0)` and `false` skips the
/// instance without breaking the parametric chain.
#[expect(
    clippy::too_many_arguments,
    reason = "11 arguments: polar-array placement takes the whole geometry + numbering spec at once"
)]
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
    let source_mm = match point_xy(source, &solve.result.state, &solve.result.index, sketch) {
        Some(p) => p,
        None => {
            warnings.push(format!(
                "polar array source {source}: position unknown — array skipped"
            ));
            return Ok(());
        }
    };
    let center_mm = match point_xy(center, &solve.result.state, &solve.result.index, sketch) {
        Some(p) => p,
        None => {
            warnings.push(format!(
                "polar array center {center}: position unknown — array skipped"
            ));
            return Ok(());
        }
    };

    let walk = Walk {
        source,
        numbering,
        params_ast,
        limit: super::MAX_ARRAY_INSTANCES,
        visit_skipped: false,
    };
    let walked = walk.polar(
        source_mm,
        center_mm,
        count_expr,
        sweep_angle_expr,
        depopulation,
        warnings,
        &mut |instance, ctx, warnings| {
            super::bake_instance(
                source, pad_attr, instance, ctx, sketch, solve, out, warnings,
            )
        },
    )?;
    if let Walked::TooMany(n) = walked {
        super::within_instance_cap("polar", source, n, warnings);
    }
    Ok(())
}
