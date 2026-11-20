//! The sketch array index `i` / `j`: parsed as references by the shared
//! parser and bound by the sketch's `EvalContext`. Moved from
//! `expr_parser.rs` when the parser moved to `signex-expr` (#1984).

use signex_sketch::expr::ast::{ArrayIndex, ExprNode};
use signex_sketch::expr::parse::parse;

#[test]
fn parse_array_index_i() {
    let e = parse("i").unwrap();
    assert_eq!(ArrayIndex::of(&e), Some(ArrayIndex::I));
}

#[test]
fn parse_array_index_j() {
    let e = parse("j").unwrap();
    assert_eq!(ArrayIndex::of(&e), Some(ArrayIndex::J));
}

#[test]
fn parse_long_ident_is_ref_not_index() {
    // Anything longer than a single 'i' or 'j' must remain a Ref —
    // the array-index reservation is exact-match only.
    for src in ["ix", "j_max", "ij", "ii", "jj"] {
        let e = parse(src).unwrap();
        match &e {
            ExprNode::Ref(name) => assert_eq!(name, src),
            other => panic!("expected Ref({src}), got {other:?}"),
        }
        assert_eq!(ArrayIndex::of(&e), None);
    }
}
