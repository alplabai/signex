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

#[test]
fn chained_references_with_full_chains_do_not_overflow_the_stack() {
    let mut table: BTreeMap<String, String> = (0..MAX_EXPR_DEPTH)
        .map(|k| {
            (
                format!("n{k}"),
                format!("= n{}{}", k + 1, "+0".repeat(MAX_EXPR_CHAIN - 1)),
            )
        })
        .collect(); // each body ~515 bytes, under MAX_EXPR_LEN
    table.insert(format!("n{MAX_EXPR_DEPTH}"), "1".to_string());
    let joined = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || eval(&parse("n0").unwrap(), &table).is_err())
        .unwrap()
        .join();
    assert!(
        joined.is_ok() && joined.unwrap(),
        "a 64-deep reference chain is refused"
    );
}

#[test]
fn an_expression_at_the_caps_naming_a_body_at_the_caps_still_evaluates() {
    // Two full-chain bodies, one reached through the other: inside the
    // budget, so the shared recursion budget does not refuse real input.
    let body = |next: &str| format!("= {next}{}", "+0".repeat(MAX_EXPR_CHAIN - 1));
    let table: BTreeMap<String, String> = [
        ("a".to_string(), body("b")),
        ("b".to_string(), body("c")),
        ("c".to_string(), "1".to_string()),
    ]
    .into_iter()
    .collect();
    let joined = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || eval(&parse("a").unwrap(), &table).map(|q| q.value))
        .unwrap()
        .join();
    assert_eq!(joined.ok(), Some(Ok(1.0)));
}
