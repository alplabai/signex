//! Shared expression-evaluation helpers for the bake passes.
//!
//! Cutout / pour / v-score / body3d all evaluate authored expressions
//! against the canonical parameter table and report failures as
//! strings; pad and array baking share only the `=`-prefix handling.

use std::collections::{BTreeMap, HashMap};

use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::unit::Quantity;

/// Strip the optional Altium-style leading `=` and surrounding
/// whitespace so authored expressions like `= pad_w` parse cleanly.
pub(crate) fn strip_eq_prefix(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s)
}

/// Build an [`EvalContext`] whose parameters are the canonical
/// millimetre values as length literals.
pub(crate) fn build_ctx(params_canonical: &HashMap<String, f64>) -> EvalContext {
    let mut params: BTreeMap<String, ExprNode> = BTreeMap::new();
    for (name, value) in params_canonical {
        params.insert(name.clone(), ExprNode::Literal(Quantity::length(*value)));
    }
    EvalContext {
        params,
        array_index: None,
    }
}

fn eval_quantity(expr: &str, ctx: &EvalContext) -> Result<Quantity, String> {
    let ast = parse(strip_eq_prefix(expr)).map_err(|e| format!("parse: {e:?}"))?;
    eval(&ast, ctx).map_err(|e| format!("eval: {e:?}"))
}

/// Evaluate a length expression to millimetres.
pub(crate) fn eval_mm(expr: &str, ctx: &EvalContext) -> Result<f64, String> {
    eval_quantity(expr, ctx)?
        .as_mm()
        .map_err(|e| format!("unit: {e:?}"))
}

/// Evaluate an optional length expression to millimetres.
pub(crate) fn opt_eval_mm(expr: &Option<String>, ctx: &EvalContext) -> Result<Option<f64>, String> {
    match expr.as_deref() {
        Some(e) => eval_mm(e, ctx).map(Some),
        None => Ok(None),
    }
}

/// Evaluate a dimensionless (count) expression.
pub(crate) fn eval_dimensionless(expr: &str, ctx: &EvalContext) -> Result<f64, String> {
    eval_quantity(expr, ctx)?
        .as_count()
        .map_err(|e| format!("unit: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> EvalContext {
        let mut p = HashMap::new();
        p.insert("w".to_string(), 2.5);
        build_ctx(&p)
    }

    #[test]
    fn strips_leading_equals_and_whitespace() {
        assert_eq!(strip_eq_prefix("  = pad_w "), "pad_w");
        assert_eq!(strip_eq_prefix("pad_w"), "pad_w");
    }

    #[test]
    fn eval_mm_resolves_params_with_or_without_prefix() {
        assert_eq!(eval_mm("= w", &ctx()), Ok(2.5));
        assert_eq!(eval_mm("w", &ctx()), Ok(2.5));
    }

    #[test]
    fn opt_eval_mm_none_and_error_paths() {
        assert_eq!(opt_eval_mm(&None, &ctx()), Ok(None));
        assert_eq!(opt_eval_mm(&Some("w".into()), &ctx()), Ok(Some(2.5)));
        assert!(
            opt_eval_mm(&Some("(".into()), &ctx())
                .unwrap_err()
                .starts_with("parse:")
        );
    }

    #[test]
    fn eval_dimensionless_rejects_length() {
        assert!(
            eval_dimensionless("w", &ctx())
                .unwrap_err()
                .starts_with("unit:")
        );
        assert_eq!(eval_dimensionless("3", &ctx()), Ok(3.0));
    }
}
