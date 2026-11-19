//! Linear array baking — `count` instances stepped by (`dx`, `dy`) from
//! the source pad.

use std::collections::BTreeMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::NumberingScheme;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use super::walk::{Walk, Walked};

/// One LinearArray expansion: walks the instances ([`Walk::linear`]) and
/// bakes each through [`super::bake_instance`].
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

    let walk = Walk {
        source,
        numbering,
        params_ast,
        limit: super::MAX_ARRAY_INSTANCES,
        visit_skipped: false,
    };
    let walked = walk.linear(
        count_expr,
        dx_expr,
        dy_expr,
        warnings,
        &mut |instance, ctx, warnings| {
            super::bake_instance(
                source, pad_attr, instance, ctx, sketch, solve, out, warnings,
            )
        },
    )?;
    if let Walked::TooMany(n) = walked {
        super::within_instance_cap("linear", source, n, warnings);
    }
    Ok(())
}
