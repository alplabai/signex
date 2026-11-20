//! Which pattern instance a baked pad is (#192 D1): the key a per-instance
//! override of that pad is stored under.

use signex_sketch::array::ArrayId;
use signex_sketch::expr::eval::EvalContext;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use super::links::Walk;
use super::overrides::attr_for;
use crate::eval_util::opt_eval_length;

/// How close (mm) a pad must sit to an instance's place to be it: well
/// under any pitch, well over the float noise of two placements.
const SAME_PLACE_MM: f64 = 1e-6;

/// The pattern instance a pad baked at `position` is, as `(pattern, i, j)`
/// keyed like [`SketchData::override_for`]: `(array, i, j)` of a plain
/// array, `(array, outer slot, inner instance)` of one that repeats another
/// pattern, `(mirror, mirrored instance, 0)` of a mirror. A plain array's
/// `(0, 0)` is its source pad. `None` when no pattern places a pad there.
/// Positions come from `solve`, as the bake reads them.
pub fn instance_at(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    position: (f64, f64),
) -> Option<(ArrayId, u32, u32)> {
    let resolved = signex_sketch::parameter::resolve(&sketch.parameters).ok()?;
    let params_ast = crate::params::params_ast(sketch, &resolved);
    let mut ignored = Vec::new();
    let patterns = sketch
        .arrays
        .iter()
        .map(|a| a.id)
        .chain(sketch.pattern_mirrors.iter().map(|m| m.id));
    for id in patterns {
        let Some(source) = sketch.innermost_source(id) else {
            continue;
        };
        let Some(pad) = sketch
            .entities
            .iter()
            .find(|e| e.id == source)
            .and_then(|e| e.pad.as_ref())
        else {
            continue;
        };
        let Some(at) = point_xy(source, &solve.result.state, &solve.result.index, sketch) else {
            continue;
        };
        let walk = Walk::new(sketch, solve, &params_ast, &mut ignored).expand(id);
        let Ok(Some(expansion)) = walk else {
            continue;
        };
        let mut ctx = EvalContext {
            params: params_ast.clone(),
            array_index: None,
        };
        for instance in expansion.instances.iter().filter(|x| x.own) {
            ctx.array_index = Some(instance.index);
            let attr = attr_for(sketch, instance, pad);
            let ox = opt_eval_length(&attr.offset_x_expr, &ctx).ok().flatten();
            let oy = opt_eval_length(&attr.offset_y_expr, &ctx).ok().flatten();
            let (x, y) = instance.place.apply(at);
            let (x, y) = (x + ox.unwrap_or(0.0), y + oy.unwrap_or(0.0));
            if (x - position.0).abs() < SAME_PLACE_MM && (y - position.1).abs() < SAME_PLACE_MM {
                return Some(instance.key);
            }
        }
    }
    None
}
