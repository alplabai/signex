//! The parameter table as the expression evaluator sees it.
//!
//! The bake entry points receive parameters as canonical `f64`s (mm, rad or a
//! plain number), which drops the unit family. A ratio, a count and an angle
//! must reach the evaluator as such, not all as lengths, so the family is
//! recovered from the sketch's own parameter table.

use std::collections::{BTreeMap, HashMap};

use signex_sketch::SketchError;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::parameter::resolve_quantities;
use signex_sketch::sketch::SketchData;
use signex_sketch::unit::{Quantity, UnitFamily};

/// One literal per canonical parameter, in the unit family the sketch's
/// parameter table declares for it. A parameter the table does not resolve
/// (or does not list) is a length, as it always was.
pub(crate) fn params_ast(
    sketch: &SketchData,
    params_canonical: &HashMap<String, f64>,
) -> BTreeMap<String, ExprNode> {
    let families: HashMap<String, UnitFamily> = resolve_quantities(&sketch.parameters)
        .map(|resolved| {
            resolved
                .into_iter()
                .map(|(name, q)| (name, q.unit.family()))
                .collect()
        })
        .unwrap_or_default();

    params_canonical
        .iter()
        .map(|(name, &value)| {
            let quantity = match families.get(name) {
                Some(UnitFamily::Angle) => Quantity::angle(value),
                Some(UnitFamily::Count) => Quantity::count(value),
                Some(UnitFamily::Length) | None => Quantity::length(value),
            };
            (name.clone(), ExprNode::Literal(quantity))
        })
        .collect()
}

/// An evaluation context over [`params_ast`], with no array index.
pub(crate) fn eval_context(
    sketch: &SketchData,
    params_canonical: &HashMap<String, f64>,
) -> EvalContext {
    EvalContext {
        params: params_ast(sketch, params_canonical),
        array_index: None,
    }
}

/// Evaluate `ast` as a length in millimetres. A unitless parameter keeps its
/// count family so it can drive a ratio, but where a length is expected it is
/// read as millimetres, the same convention the solver applies to a dimension
/// target (`solver::residual::resolve_dim`). A unitless *result* (a bare
/// literal such as `"0.5"`) is read as millimetres for the same reason.
pub(crate) fn eval_length_mm(ast: &ExprNode, ctx: &EvalContext) -> Result<f64, SketchError> {
    let as_length_mm = |q: Quantity| {
        q.as_mm()
            .or_else(|err| q.as_count().map_err(|_| err))
            .map_err(SketchError::Unit)
    };
    let typed = eval(ast, ctx)
        .map_err(SketchError::Expr)
        .and_then(as_length_mm);
    match typed {
        Ok(mm) => Ok(mm),
        Err(first) => {
            let lenient = ctx.with_unitless_as_mm();
            if lenient.params == ctx.params {
                return Err(first);
            }
            eval(ast, &lenient)
                .map_err(SketchError::Expr)
                .and_then(as_length_mm)
                .map_err(|_| first)
        }
    }
}

/// The error text the string-returning evaluators report.
pub(crate) fn describe(error: SketchError) -> String {
    match error {
        SketchError::Expr(e) => format!("eval: {e:?}"),
        SketchError::Unit(e) => format!("unit: {e:?}"),
        other => format!("{other:?}"),
    }
}
