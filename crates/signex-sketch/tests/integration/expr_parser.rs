//! Integration tests for the recursive-descent expression parser
//! (`crates/signex-sketch/src/expr/parse.rs`).
//!
//! Covers Task 4.3 of `docs/internal/SKETCH_MODE_v0.13_PLAN.md`.

use signex_sketch::expr::ast::{ArrayIndex, BinOp, ExprNode, UnaryOp};
use signex_sketch::expr::parse::parse;
use signex_sketch::unit::{Quantity, Unit};

const EPS: f64 = 1e-12;

// -- helpers --------------------------------------------------------------

/// Pull the inner [`Quantity`] out of an [`ExprNode::Literal`], or
/// panic with a descriptive message.
fn lit(node: &ExprNode) -> Quantity {
    match node {
        ExprNode::Literal(q) => *q,
        other => panic!("expected Literal, got {other:?}"),
    }
}

// -- literals -------------------------------------------------------------

#[test]
fn parse_literal_mm() {
    let e = parse("0.5mm").unwrap();
    let q = lit(&e);
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 0.5).abs() < EPS);
}

#[test]
fn parse_literal_dimensionless() {
    let e = parse("16").unwrap();
    let q = lit(&e);
    assert_eq!(q.unit, Unit::Dimensionless);
    assert!((q.value - 16.0).abs() < EPS);
}

// -- arithmetic / precedence / associativity ------------------------------

#[test]
fn parse_addition_with_units() {
    let e = parse("0.5mm + 100mil").unwrap();
    match e {
        ExprNode::Binary(BinOp::Add, lhs, rhs) => {
            assert_eq!(lit(&lhs).unit, Unit::Mm);
            assert_eq!(lit(&rhs).unit, Unit::Mil);
        }
        other => panic!("expected Binary(Add, ...), got {other:?}"),
    }
}

#[test]
fn parse_left_assoc() {
    // 1 - 2 - 3 should fold left:  ((1 - 2) - 3)
    let e = parse("1 - 2 - 3").unwrap();
    match e {
        ExprNode::Binary(BinOp::Sub, lhs, rhs) => {
            // outer rhs is a literal 3
            assert_eq!(lit(&rhs).value, 3.0);
            // outer lhs is (1 - 2)
            match *lhs {
                ExprNode::Binary(BinOp::Sub, ll, lr) => {
                    assert_eq!(lit(&ll).value, 1.0);
                    assert_eq!(lit(&lr).value, 2.0);
                }
                other => panic!("expected nested Binary(Sub), got {other:?}"),
            }
        }
        other => panic!("expected outer Binary(Sub), got {other:?}"),
    }
}

#[test]
fn parse_right_assoc_pow() {
    // 2 ^ 3 ^ 2 should fold right:  2 ^ (3 ^ 2)  ==  2 ^ 9 == 512
    let e = parse("2 ^ 3 ^ 2").unwrap();
    match e {
        ExprNode::Binary(BinOp::Pow, lhs, rhs) => {
            assert_eq!(lit(&lhs).value, 2.0);
            // rhs must itself be Binary(Pow, 3, 2)
            match *rhs {
                ExprNode::Binary(BinOp::Pow, rl, rr) => {
                    assert_eq!(lit(&rl).value, 3.0);
                    assert_eq!(lit(&rr).value, 2.0);
                }
                other => panic!("expected nested Binary(Pow), got {other:?}"),
            }
        }
        other => panic!("expected outer Binary(Pow), got {other:?}"),
    }
}

#[test]
fn parse_precedence() {
    // 1 + 2 * 3  ==>  Add(1, Mul(2, 3))
    let e = parse("1 + 2*3").unwrap();
    match e {
        ExprNode::Binary(BinOp::Add, lhs, rhs) => {
            assert_eq!(lit(&lhs).value, 1.0);
            match *rhs {
                ExprNode::Binary(BinOp::Mul, rl, rr) => {
                    assert_eq!(lit(&rl).value, 2.0);
                    assert_eq!(lit(&rr).value, 3.0);
                }
                other => panic!("expected Binary(Mul), got {other:?}"),
            }
        }
        other => panic!("expected Binary(Add), got {other:?}"),
    }
}

#[test]
fn parse_parens() {
    // (1 + 2) * 3  ==>  Mul(Add(1, 2), 3)
    let e = parse("(1+2)*3").unwrap();
    match e {
        ExprNode::Binary(BinOp::Mul, lhs, rhs) => {
            assert_eq!(lit(&rhs).value, 3.0);
            match *lhs {
                ExprNode::Binary(BinOp::Add, ll, lr) => {
                    assert_eq!(lit(&ll).value, 1.0);
                    assert_eq!(lit(&lr).value, 2.0);
                }
                other => panic!("expected Binary(Add), got {other:?}"),
            }
        }
        other => panic!("expected Binary(Mul), got {other:?}"),
    }
}

// -- unary ----------------------------------------------------------------

#[test]
fn parse_unary_neg() {
    let e = parse("-5mm").unwrap();
    match e {
        ExprNode::Unary(UnaryOp::Neg, inner) => {
            let q = lit(&inner);
            assert_eq!(q.unit, Unit::Mm);
            assert!((q.value - 5.0).abs() < EPS);
        }
        other => panic!("expected Unary(Neg), got {other:?}"),
    }
}

#[test]
fn parse_unary_not() {
    let e = parse("!cond").unwrap();
    match e {
        ExprNode::Unary(UnaryOp::Not, inner) => match *inner {
            ExprNode::Ref(name) => assert_eq!(name, "cond"),
            other => panic!("expected Ref, got {other:?}"),
        },
        other => panic!("expected Unary(Not), got {other:?}"),
    }
}

// -- references / array index --------------------------------------------

#[test]
fn parse_ref() {
    let e = parse("pad_pitch").unwrap();
    match e {
        ExprNode::Ref(name) => assert_eq!(name, "pad_pitch"),
        other => panic!("expected Ref, got {other:?}"),
    }
}

#[test]
fn parse_array_index_i() {
    let e = parse("i").unwrap();
    assert!(matches!(e, ExprNode::ArrayIndex(ArrayIndex::I)));
}

#[test]
fn parse_array_index_j() {
    let e = parse("j").unwrap();
    assert!(matches!(e, ExprNode::ArrayIndex(ArrayIndex::J)));
}

#[test]
fn parse_long_ident_is_ref_not_index() {
    // Anything longer than a single 'i' or 'j' must remain a Ref —
    // the array-index reservation is exact-match only.
    for src in ["ix", "j_max", "ij", "ii", "jj"] {
        let e = parse(src).unwrap();
        match e {
            ExprNode::Ref(name) => assert_eq!(name, src),
            other => panic!("expected Ref({src}), got {other:?}"),
        }
    }
}

// -- ternary / lookup / mixed --------------------------------------------

#[test]
fn parse_ternary() {
    let e = parse("cond ? a : b").unwrap();
    match e {
        ExprNode::Ternary(c, t, el) => {
            assert!(matches!(*c, ExprNode::Ref(ref n) if n == "cond"));
            assert!(matches!(*t, ExprNode::Ref(ref n) if n == "a"));
            assert!(matches!(*el, ExprNode::Ref(ref n) if n == "b"));
        }
        other => panic!("expected Ternary, got {other:?}"),
    }
}

#[test]
fn parse_lookup() {
    let e = parse("lookup(p, [16, 32], [1mm, 2mm])").unwrap();
    match e {
        ExprNode::Lookup { key, keys, values } => {
            assert!(matches!(*key, ExprNode::Ref(ref n) if n == "p"));
            assert_eq!(keys.len(), 2);
            assert_eq!(values.len(), 2);
            assert_eq!(lit(&keys[0]).value, 16.0);
            assert_eq!(lit(&keys[1]).value, 32.0);
            assert_eq!(lit(&values[0]).unit, Unit::Mm);
            assert_eq!(lit(&values[0]).value, 1.0);
            assert_eq!(lit(&values[1]).unit, Unit::Mm);
            assert_eq!(lit(&values[1]).value, 2.0);
        }
        other => panic!("expected Lookup, got {other:?}"),
    }
}

#[test]
fn parse_compare_and_logic() {
    // (a > 5) && (b < 10)  ==>  And(Gt(a, 5), Lt(b, 10))
    let e = parse("(a > 5) && (b < 10)").unwrap();
    match e {
        ExprNode::Binary(BinOp::And, lhs, rhs) => {
            match *lhs {
                ExprNode::Binary(BinOp::Gt, ll, lr) => {
                    assert!(matches!(*ll, ExprNode::Ref(ref n) if n == "a"));
                    assert_eq!(lit(&lr).value, 5.0);
                }
                other => panic!("expected Binary(Gt), got {other:?}"),
            }
            match *rhs {
                ExprNode::Binary(BinOp::Lt, rl, rr) => {
                    assert!(matches!(*rl, ExprNode::Ref(ref n) if n == "b"));
                    assert_eq!(lit(&rr).value, 10.0);
                }
                other => panic!("expected Binary(Lt), got {other:?}"),
            }
        }
        other => panic!("expected Binary(And), got {other:?}"),
    }
}

// -- error paths ----------------------------------------------------------

#[test]
fn parse_invalid_unit_fails() {
    // "xyz" is not a known unit suffix; the unit parser reports
    // BadQuantity, which propagates through the ExprError::From impl.
    assert!(parse("0.5xyz + 1").is_err());
}

#[test]
fn parse_unbalanced_paren_fails() {
    assert!(parse("(1 + 2").is_err());
}

#[test]
fn parse_lookup_length_mismatch_fails() {
    // 2 keys, 3 values
    assert!(parse("lookup(p, [1, 2], [1mm, 2mm, 3mm])").is_err());
    // 3 keys, 1 value
    assert!(parse("lookup(p, [1, 2, 3], [1mm])").is_err());
}

#[test]
fn parse_empty_fails() {
    assert!(parse("").is_err());
    assert!(parse("   ").is_err());
}
