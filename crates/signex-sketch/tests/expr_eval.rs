//! Integration tests for the expression evaluator
//! (`crates/signex-sketch/src/expr/eval.rs`).
//!
//! Covers Task 4.4 of `docs/internal/SKETCH_MODE_v0.13_PLAN.md`.
//!
//! ASTs are built by hand because the recursive-descent parser
//! (Task 4.3) is being implemented in a sibling agent and may not
//! have landed yet. Once it does, these tests can be rewritten with
//! `parse(...)`; the evaluator semantics they assert are independent.

use std::collections::BTreeMap;

use signex_sketch::expr::ast::{ArrayIndex, BinOp, ExprNode, UnaryOp};
use signex_sketch::expr::eval::{eval, EvalContext};
use signex_sketch::expr::ExprError;
use signex_sketch::unit::{Quantity, Unit};

const EPS: f64 = 1e-10;

// ---------------------------------------------------------------------
// AST builder helpers
// ---------------------------------------------------------------------

fn lit_mm(v: f64) -> ExprNode {
    ExprNode::Literal(Quantity::length(v))
}

fn lit(v: f64, u: Unit) -> ExprNode {
    ExprNode::Literal(Quantity { value: v, unit: u })
}

fn lit_count(v: f64) -> ExprNode {
    ExprNode::Literal(Quantity::count(v))
}

fn r#ref(name: &str) -> ExprNode {
    ExprNode::Ref(name.to_string())
}

fn bin(op: BinOp, l: ExprNode, r: ExprNode) -> ExprNode {
    ExprNode::Binary(op, Box::new(l), Box::new(r))
}

fn una(op: UnaryOp, e: ExprNode) -> ExprNode {
    ExprNode::Unary(op, Box::new(e))
}

fn ternary(c: ExprNode, t: ExprNode, f: ExprNode) -> ExprNode {
    ExprNode::Ternary(Box::new(c), Box::new(t), Box::new(f))
}

fn ctx_with(params: &[(&str, ExprNode)]) -> EvalContext {
    let mut p = BTreeMap::new();
    for (name, node) in params {
        p.insert((*name).to_string(), node.clone());
    }
    EvalContext {
        params: p,
        array_index: None,
    }
}

// ---------------------------------------------------------------------
// Spec-listed cases
// ---------------------------------------------------------------------

#[test]
fn eval_literal_mm() {
    // "0.5mm" → 0.5 mm
    let q = eval(&lit_mm(0.5), &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 0.5).abs() < EPS);
}

#[test]
fn eval_addition_unit_conversion() {
    // "0.5mm + 100mil" → 3.04 mm   (100mil = 2.54mm)
    let e = bin(BinOp::Add, lit_mm(0.5), lit(100.0, Unit::Mil));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 3.04).abs() < EPS, "got {}", q.value);
}

#[test]
fn eval_param_ref() {
    // "pad_pitch * 5" with pad_pitch = 0.5mm → 2.5 mm
    let e = bin(BinOp::Mul, r#ref("pad_pitch"), lit_count(5.0));
    let ctx = ctx_with(&[("pad_pitch", lit_mm(0.5))]);
    let q = eval(&e, &ctx).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 2.5).abs() < EPS);
}

#[test]
fn eval_ternary_takes_then() {
    // "i == 0 ? 0.4mm : 0.25mm" with array_index=Some((0,0)) → 0.4mm
    let e = ternary(
        bin(
            BinOp::Eq,
            ExprNode::ArrayIndex(ArrayIndex::I),
            lit_count(0.0),
        ),
        lit_mm(0.4),
        lit_mm(0.25),
    );
    let ctx = EvalContext {
        params: BTreeMap::new(),
        array_index: Some((0, 0)),
    };
    let q = eval(&e, &ctx).unwrap();
    assert!((q.value - 0.4).abs() < EPS);
}

#[test]
fn eval_ternary_takes_else() {
    // Same expression, array_index=Some((1,0)) → 0.25 mm
    let e = ternary(
        bin(
            BinOp::Eq,
            ExprNode::ArrayIndex(ArrayIndex::I),
            lit_count(0.0),
        ),
        lit_mm(0.4),
        lit_mm(0.25),
    );
    let ctx = EvalContext {
        params: BTreeMap::new(),
        array_index: Some((1, 0)),
    };
    let q = eval(&e, &ctx).unwrap();
    assert!((q.value - 0.25).abs() < EPS);
}

#[test]
fn eval_lookup_match() {
    // lookup(p, [0, 1], [0.5mm, 0.8mm]) with p = 1 → 0.8 mm
    let e = ExprNode::Lookup {
        key: Box::new(r#ref("p")),
        keys: vec![lit_count(0.0), lit_count(1.0)],
        values: vec![lit_mm(0.5), lit_mm(0.8)],
    };
    let ctx = ctx_with(&[("p", lit_count(1.0))]);
    let q = eval(&e, &ctx).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 0.8).abs() < EPS);
}

#[test]
fn eval_lookup_no_match_errors() {
    // Same lookup with p = 42 → LookupNoMatch
    let e = ExprNode::Lookup {
        key: Box::new(r#ref("p")),
        keys: vec![lit_count(0.0), lit_count(1.0)],
        values: vec![lit_mm(0.5), lit_mm(0.8)],
    };
    let ctx = ctx_with(&[("p", lit_count(42.0))]);
    assert_eq!(eval(&e, &ctx), Err(ExprError::LookupNoMatch));
}

#[test]
fn eval_unary_neg() {
    // "-5mm" → -5 mm (unit preserved)
    let e = una(UnaryOp::Neg, lit_mm(5.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value + 5.0).abs() < EPS);
}

#[test]
fn eval_unit_mismatch_errors() {
    // "0.5mm + 90deg" → UnitMismatch
    let e = bin(BinOp::Add, lit_mm(0.5), lit(90.0, Unit::Deg));
    let result = eval(&e, &EvalContext::default());
    assert!(
        matches!(result, Err(ExprError::UnitMismatch { .. })),
        "expected UnitMismatch, got {result:?}"
    );
}

#[test]
fn eval_unknown_param_errors() {
    // Reference to a parameter that's not in the table → Unknown
    let e = r#ref("x");
    let result = eval(&e, &EvalContext::default());
    assert_eq!(result, Err(ExprError::Unknown("x".to_string())));
}

#[test]
fn eval_array_index_in_context() {
    // "i" with array_index = Some((3, 0)) → 3 (Dimensionless)
    let e = ExprNode::ArrayIndex(ArrayIndex::I);
    let ctx = EvalContext {
        params: BTreeMap::new(),
        array_index: Some((3, 0)),
    };
    let q = eval(&e, &ctx).unwrap();
    assert_eq!(q, Quantity::count(3.0));
}

#[test]
fn eval_array_index_outside_errors() {
    // "i" with array_index = None → ArrayIndexOutsideArray
    let e = ExprNode::ArrayIndex(ArrayIndex::I);
    let result = eval(&e, &EvalContext::default());
    assert_eq!(result, Err(ExprError::ArrayIndexOutsideArray));
}

#[test]
fn eval_compare_returns_dimensionless() {
    // "5 > 3" → 1 (Dimensionless)
    let e = bin(BinOp::Gt, lit_count(5.0), lit_count(3.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(1.0));

    // And "5 < 3" → 0
    let e = bin(BinOp::Lt, lit_count(5.0), lit_count(3.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(0.0));
}

#[test]
fn eval_logical_and_or() {
    // "(1) && (0)" → 0
    let e = bin(BinOp::And, lit_count(1.0), lit_count(0.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(0.0));

    // "(1) || (0)" → 1
    let e = bin(BinOp::Or, lit_count(1.0), lit_count(0.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(1.0));
}

#[test]
fn eval_pow_dimensionless() {
    // "2 ^ 8" → 256
    let e = bin(BinOp::Pow, lit_count(2.0), lit_count(8.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(256.0));
}

// ---------------------------------------------------------------------
// Additional coverage: mul / div / mod / unary-not / lookup-shape /
// ref recursion / cross-family cmp / pow-on-length
// ---------------------------------------------------------------------

#[test]
fn eval_mul_length_times_count_keeps_length_unit() {
    // 0.5mm * 4 → 2.0 mm
    let e = bin(BinOp::Mul, lit_mm(0.5), lit_count(4.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 2.0).abs() < EPS);
}

#[test]
fn eval_mul_length_times_length_errors() {
    // mm × mm would need an Area unit → UnitMismatch
    let e = bin(BinOp::Mul, lit_mm(2.0), lit_mm(3.0));
    assert!(matches!(
        eval(&e, &EvalContext::default()),
        Err(ExprError::UnitMismatch { .. })
    ));
}

#[test]
fn eval_div_length_by_length_returns_count() {
    // 6mm / 2mm → 3 (Dimensionless)
    let e = bin(BinOp::Div, lit_mm(6.0), lit_mm(2.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Dimensionless);
    assert!((q.value - 3.0).abs() < EPS);
}

#[test]
fn eval_div_length_by_length_with_unit_conversion() {
    // 1mm / 1mil = 1 / 0.0254 ≈ 39.3700787...
    let e = bin(BinOp::Div, lit_mm(1.0), lit(1.0, Unit::Mil));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Dimensionless);
    assert!((q.value - (1.0 / 0.0254)).abs() < EPS, "got {}", q.value);
}

#[test]
fn eval_mod_length_by_length() {
    // 7mm mod 3mm → 1 (Dimensionless)
    let e = bin(BinOp::Mod, lit_mm(7.0), lit_mm(3.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Dimensionless);
    assert!((q.value - 1.0).abs() < EPS);
}

#[test]
fn eval_unary_not_on_zero_returns_one() {
    let e = una(UnaryOp::Not, lit_count(0.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(1.0));
}

#[test]
fn eval_unary_not_on_nonzero_returns_zero() {
    let e = una(UnaryOp::Not, lit_count(7.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(0.0));
}

#[test]
fn eval_unary_not_on_unit_errors() {
    // !5mm — operand must be Dimensionless
    let e = una(UnaryOp::Not, lit_mm(5.0));
    let result = eval(&e, &EvalContext::default());
    assert!(matches!(result, Err(ExprError::WrongFamily { .. })));
}

#[test]
fn eval_lookup_shape_mismatch() {
    // 2 keys, 1 value → LookupShapeMismatch
    let e = ExprNode::Lookup {
        key: Box::new(lit_count(0.0)),
        keys: vec![lit_count(0.0), lit_count(1.0)],
        values: vec![lit_mm(0.5)],
    };
    assert_eq!(
        eval(&e, &EvalContext::default()),
        Err(ExprError::LookupShapeMismatch)
    );
}

#[test]
fn eval_lookup_with_unit_conversion_in_keys() {
    // Lookup key in mm, the table key in mil; equal-canonical match.
    // 2.54mm == 100mil
    let e = ExprNode::Lookup {
        key: Box::new(lit_mm(2.54)),
        keys: vec![lit(100.0, Unit::Mil), lit(200.0, Unit::Mil)],
        values: vec![lit_count(7.0), lit_count(8.0)],
    };
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(7.0));
}

#[test]
fn eval_ref_chains_recursively() {
    // a = b * 2; b = 3mm   → a = 6 mm
    let a = bin(BinOp::Mul, r#ref("b"), lit_count(2.0));
    let ctx = ctx_with(&[("a", a.clone()), ("b", lit_mm(3.0))]);
    let q = eval(&r#ref("a"), &ctx).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 6.0).abs() < EPS);
}

#[test]
fn eval_compare_cross_family_errors() {
    // 1mm == 90deg → UnitMismatch (different families)
    let e = bin(BinOp::Eq, lit_mm(1.0), lit(90.0, Unit::Deg));
    assert!(matches!(
        eval(&e, &EvalContext::default()),
        Err(ExprError::UnitMismatch { .. })
    ));
}

#[test]
fn eval_compare_eq_with_tolerance() {
    // 0.5mm == 500um (canonical equal) → 1
    let e = bin(BinOp::Eq, lit_mm(0.5), lit(500.0, Unit::Um));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q, Quantity::count(1.0));
}

#[test]
fn eval_pow_length_to_one_is_identity() {
    // 5mm ^ 1 → 5mm
    let e = bin(BinOp::Pow, lit_mm(5.0), lit_count(1.0));
    let q = eval(&e, &EvalContext::default()).unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert!((q.value - 5.0).abs() < EPS);
}

#[test]
fn eval_pow_length_to_two_errors() {
    // 5mm ^ 2 would need Area; not modelled in v0.13.
    let e = bin(BinOp::Pow, lit_mm(5.0), lit_count(2.0));
    assert!(matches!(
        eval(&e, &EvalContext::default()),
        Err(ExprError::Domain(_))
    ));
}

#[test]
fn eval_pow_with_unit_exponent_errors() {
    // 2 ^ 90deg — exponent must be Dimensionless.
    let e = bin(BinOp::Pow, lit_count(2.0), lit(90.0, Unit::Deg));
    assert!(matches!(
        eval(&e, &EvalContext::default()),
        Err(ExprError::UnitMismatch { .. })
    ));
}

#[test]
fn eval_array_index_j() {
    // "j" with array_index = Some((3, 7)) → 7
    let e = ExprNode::ArrayIndex(ArrayIndex::J);
    let ctx = EvalContext {
        params: BTreeMap::new(),
        array_index: Some((3, 7)),
    };
    let q = eval(&e, &ctx).unwrap();
    assert_eq!(q, Quantity::count(7.0));
}
