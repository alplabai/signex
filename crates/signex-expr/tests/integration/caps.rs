//! The input caps (#678): length, nesting depth, operator chain, and the
//! reference-chain depth the evaluator follows.

use std::collections::BTreeMap;

use signex_expr::{ExprError, MAX_EXPR_CHAIN, MAX_EXPR_DEPTH, MAX_EXPR_LEN, eval, parse};

#[test]
fn source_longer_than_the_cap_is_refused() {
    let src = "1".repeat(MAX_EXPR_LEN + 1);
    assert!(matches!(parse(&src), Err(ExprError::TooLong { .. })));
}

#[test]
fn nesting_deeper_than_the_cap_is_refused() {
    let src = format!(
        "{}1{}",
        "(".repeat(MAX_EXPR_DEPTH + 1),
        ")".repeat(MAX_EXPR_DEPTH + 1)
    );
    assert!(matches!(parse(&src), Err(ExprError::TooDeep { .. })));
}

#[test]
fn a_chain_longer_than_the_cap_is_refused() {
    let src = ["1"; MAX_EXPR_CHAIN + 2].join("+");
    assert!(matches!(parse(&src), Err(ExprError::ChainTooLong { .. })));
}

#[test]
fn a_reference_chain_deeper_than_the_cap_is_refused() {
    // n0 = n1, n1 = n2, ... : no cycle, but deeper than the evaluator follows.
    let table: BTreeMap<String, String> = (0..=MAX_EXPR_DEPTH + 1)
        .map(|k| (format!("n{k}"), format!("= n{}", k + 1)))
        .collect();
    assert!(matches!(
        eval(&parse("n0").unwrap(), &table),
        Err(ExprError::TooDeep { .. })
    ));
}
