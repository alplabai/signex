//! #678 — expressions, dimension literals and nesting depth are validated
//! at the sketch boundary: a non-finite number or an absurdly deep
//! expression is a reported error, never NaN geometry or a stack overflow.

use signex_sketch::SketchError;
use signex_sketch::constraint::{DimTarget, parse_dimension_input};
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::expr::{ExprError, MAX_EXPR_DEPTH};
use signex_sketch::solver::residual::{ResolvedParams, resolve_dim};
use signex_sketch::unit::parse_quantity;

fn eval_str(src: &str) -> Result<f64, ExprError> {
    let ast = parse(src)?;
    eval(&ast, &EvalContext::default()).map(|q| q.value)
}

#[test]
fn quantity_literals_reject_non_finite() {
    for bad in [
        "nan", "NaN", "inf", "-inf", "infinity", "1e999", "1e999mm", "infmm", "nanmm",
    ] {
        assert!(parse_quantity(bad).is_err(), "{bad:?} must not parse");
    }
    assert_eq!(parse_quantity("2mm").unwrap().value, 2.0);
}

#[test]
fn expression_with_overflowing_literal_is_rejected() {
    let huge = format!("1{}", "0".repeat(400));
    assert!(eval_str(&huge).is_err());
    assert!(eval_str(&format!("{huge}mm")).is_err());
}

#[test]
fn power_with_nan_result_is_rejected() {
    // (0-1)^0.5 is NaN in IEEE arithmetic.
    assert!(matches!(
        eval_str("(0-1)^0.5"),
        Err(ExprError::NonFinite(_))
    ));
}

#[test]
fn power_overflowing_to_infinity_is_rejected() {
    assert!(matches!(eval_str("10^400"), Err(ExprError::NonFinite(_))));
}

#[test]
fn length_expression_driven_by_nan_power_is_rejected() {
    // The parameter shape from the issue: `w = 2mm * (0-1)^0.5`.
    assert!(matches!(
        eval_str("2mm * (0-1)^0.5"),
        Err(ExprError::NonFinite(_))
    ));
}

#[test]
fn finite_expressions_still_evaluate() {
    assert_eq!(eval_str("2mm * 3").unwrap(), 6.0);
    assert_eq!(eval_str("2^10").unwrap(), 1024.0);
}

#[test]
fn literal_dimension_target_rejects_non_finite() {
    let params = ResolvedParams::default();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            matches!(
                resolve_dim(&DimTarget::Literal(bad), &params),
                Err(SketchError::Expr(ExprError::NonFinite(_)))
            ),
            "{bad} must be rejected"
        );
    }
    assert_eq!(resolve_dim(&DimTarget::Literal(1.5), &params).unwrap(), 1.5);
}

#[test]
fn dimension_input_text_rejects_non_finite_and_garbage() {
    for bad in [
        "nan", "NaN", "inf", "-inf", "infinity", "1e999", "1,5", "abc",
    ] {
        assert!(
            parse_dimension_input(bad).is_err(),
            "{bad:?} must be rejected"
        );
    }
    assert_eq!(parse_dimension_input(" 1.5 ").unwrap(), 1.5);
    assert_eq!(parse_dimension_input("-2").unwrap(), -2.0);
}

#[test]
fn deeply_nested_parentheses_are_rejected_not_overflowed() {
    let n = MAX_EXPR_DEPTH + 40;
    let src = format!("{}1{}", "(".repeat(n), ")".repeat(n));
    assert!(matches!(parse(&src), Err(ExprError::TooDeep { .. })));
}

#[test]
fn enormous_nesting_does_not_abort_the_process() {
    let n = 200_000;
    let src = format!("{}1{}", "(".repeat(n), ")".repeat(n));
    assert!(parse(&src).is_err());
}

#[test]
fn long_unary_chains_are_rejected_not_overflowed() {
    // Under the length cap, so the depth guard is what stops it.
    assert!(matches!(
        parse(&format!("{}1", "-".repeat(1000))),
        Err(ExprError::TooDeep { .. })
    ));
    assert!(matches!(
        parse(&format!("{}1", "!".repeat(1000))),
        Err(ExprError::TooDeep { .. })
    ));
    // Far over the length cap: still an error, never an abort.
    assert!(parse(&format!("{}1", "-".repeat(500_000))).is_err());
}

#[test]
fn nesting_at_the_limit_still_parses() {
    // Each parenthesis group costs two levels (expression + unary).
    let n = MAX_EXPR_DEPTH / 2 - 2;
    let src = format!("{}1{}", "(".repeat(n), ")".repeat(n));
    assert!(parse(&src).is_ok());
}

#[test]
fn overlong_flat_chain_is_rejected() {
    let src = vec!["1"; 100_000].join("+");
    assert!(matches!(parse(&src), Err(ExprError::TooLong { .. })));
}
