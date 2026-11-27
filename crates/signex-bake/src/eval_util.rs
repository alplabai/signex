//! Shared expression-evaluation helpers for the bake passes.
//!
//! Cutout / pour / v-score / body3d all evaluate authored expressions
//! against the canonical parameter table and report failures as
//! strings; pad and array baking share only the `=`-prefix handling.

use std::collections::HashMap;

use signex_expr::{LiteralContext, Quantity, parse, parse_with};
use signex_sketch::SketchError;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::sketch::SketchData;

/// Strip the optional leading `=` and surrounding whitespace so authored
/// expressions like `= pad_w` parse cleanly (the sketch-style rule,
/// [`signex_expr::expression_body`]).
pub(crate) fn strip_eq_prefix(src: &str) -> &str {
    signex_expr::expression_body(src)
}

/// Parse a value that must be a length: a bare `m` reads as metres here
/// ([`signex_expr::bare_m_meaning`]).
fn parse_length(src: &str) -> Result<signex_expr::ExprNode, signex_expr::ExprError> {
    parse_with(strip_eq_prefix(src), LiteralContext::Length)
}

/// Parse an authored field in the context its value must have: a pitch in
/// [`LiteralContext::Length`], a count in [`LiteralContext::Count`], a
/// rotation or a sweep in [`LiteralContext::Angle`], so `2.54m` is metres
/// and `4k7` is never a multiplier (#2683).
pub(crate) fn parse_in(
    src: &str,
    context: LiteralContext,
) -> Result<signex_expr::ExprNode, SketchError> {
    parse_with(strip_eq_prefix(src), context).map_err(SketchError::Expr)
}

/// Build an [`EvalContext`] whose parameters are the canonical values in the
/// unit family the sketch's parameter table declares for each.
pub(crate) fn build_ctx(
    sketch: &SketchData,
    params_canonical: &HashMap<String, f64>,
) -> EvalContext {
    crate::params::eval_context(sketch, params_canonical)
}

fn eval_quantity(expr: &str, ctx: &EvalContext) -> Result<Quantity, String> {
    let ast = parse(strip_eq_prefix(expr)).map_err(|e| format!("parse: {e:?}"))?;
    eval(&ast, ctx).map_err(|e| format!("eval: {e:?}"))
}

/// Evaluate a length expression to millimetres. A unitless parameter is read
/// as millimetres here (see [`crate::params::eval_length_mm`]).
pub(crate) fn eval_mm(expr: &str, ctx: &EvalContext) -> Result<f64, String> {
    let ast = parse_length(expr).map_err(|e| format!("parse: {e:?}"))?;
    crate::params::eval_length_mm(&ast, ctx).map_err(crate::params::describe)
}

/// [`eval_mm`] with the typed error, for callers that propagate
/// [`SketchError`] (pad baking). Same parse + unitless-as-mm rule.
pub(crate) fn eval_length(expr: &str, ctx: &EvalContext) -> Result<f64, SketchError> {
    let ast = parse_length(expr).map_err(SketchError::Expr)?;
    crate::params::eval_length_mm(&ast, ctx)
}

/// Optional counterpart of [`eval_length`].
pub(crate) fn opt_eval_length(
    expr: &Option<String>,
    ctx: &EvalContext,
) -> Result<Option<f64>, SketchError> {
    expr.as_deref().map(|e| eval_length(e, ctx)).transpose()
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
        build_ctx(&SketchData::default(), &p)
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

    #[test]
    fn ten_m_via_a_parameter_matches_the_field_or_errors() {
        let mut sketch = SketchData::default();
        sketch.parameters.insert("pad_w", "10m");
        let direct = eval_mm("10m", &ctx());
        let via_param = signex_sketch::parameter::resolve(&sketch.parameters)
            .map_err(|e| format!("resolve: {e:?}"))
            .and_then(|p| eval_mm("= pad_w", &build_ctx(&sketch, &p)));
        assert_eq!(direct, Ok(10_000.0));
        assert!(
            via_param.is_err() || via_param == direct,
            "via parameter {via_param:?}, direct {direct:?}"
        );
    }

    #[test]
    fn prefix_only_literals_via_a_parameter_match_the_field_or_error() {
        for literal in ["1u", "4k7", "2n", "10m", "500m", "2n2"] {
            let mut sketch = SketchData::default();
            sketch.parameters.insert("pad_w", literal);
            let direct = eval_mm(literal, &ctx());
            let via_param = signex_sketch::parameter::resolve(&sketch.parameters)
                .map_err(|e| format!("resolve: {e:?}"))
                .and_then(|p| eval_mm("= pad_w", &build_ctx(&sketch, &p)));
            assert!(
                via_param.is_err() || via_param == direct,
                "{literal}: via parameter {via_param:?}, direct {direct:?}"
            );
        }
    }

    #[test]
    fn a_parameter_with_a_unit_still_bakes() {
        let mut sketch = SketchData::default();
        sketch.parameters.insert("pad_w", "0.5mm");
        sketch.parameters.insert("pins", "16");
        let p = signex_sketch::parameter::resolve(&sketch.parameters).unwrap();
        assert_eq!(eval_mm("= pad_w", &build_ctx(&sketch, &p)), Ok(0.5));
        assert_eq!(
            eval_dimensionless("= pins", &build_ctx(&sketch, &p)),
            Ok(16.0)
        );
    }
}
