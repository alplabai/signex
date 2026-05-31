//! Expression AST for the parametric sketch parameter table.
//!
//! The AST is constructed by the parser (`crate::expr::parse`) from the
//! source `String` stored in `ParameterTable`. The AST itself is *not*
//! serialised; the persisted form is the source string, and the AST is
//! rebuilt by re-parsing on load. This keeps the on-disk format simple
//! and human-editable.
//!
//! # Design
//!
//! Standard pure-functional expression tree:
//!
//! - [`ExprNode::Literal`] — a [`Quantity`] (value + unit)
//! - [`ExprNode::Ref`] — named parameter reference
//! - [`ExprNode::ArrayIndex`] — `i` / `j` inside an array's per-instance
//!   expressions or depopulation mask
//! - [`ExprNode::Binary`] — infix binary operators (arithmetic,
//!   comparison, logical)
//! - [`ExprNode::Unary`] — prefix unary operators (negate, logical not)
//! - [`ExprNode::Ternary`] — `cond ? then : else`
//! - [`ExprNode::Lookup`] — table lookup, used for parametric pad-shape
//!   selection by package family

use crate::unit::Quantity;

/// One node of an expression tree.
///
/// All variants carry their own children boxed where recursion is
/// needed, so an `ExprNode` is `Sized` and small enough to pass by
/// value without indirection at the top level.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprNode {
    /// Literal value with a unit attached (e.g. `0.5mm`, `90deg`,
    /// `16`).
    Literal(Quantity),
    /// Reference to a parameter by name (e.g. `pad_pitch`).
    Ref(String),
    /// Array index — `i` or `j` inside an array's depopulation mask
    /// or per-instance expression.
    ArrayIndex(ArrayIndex),
    /// Binary infix operator.
    Binary(BinOp, Box<ExprNode>, Box<ExprNode>),
    /// Prefix unary operator.
    Unary(UnaryOp, Box<ExprNode>),
    /// `cond ? then : else`.
    Ternary(Box<ExprNode>, Box<ExprNode>, Box<ExprNode>),
    /// Table lookup — `lookup(key, [k1, k2, ...], [v1, v2, ...])`.
    /// Returns the value whose key matches; the evaluator errors if no
    /// match is found. `keys.len() == values.len()` is a parser-level
    /// invariant.
    Lookup {
        key: Box<ExprNode>,
        keys: Vec<ExprNode>,
        values: Vec<ExprNode>,
    },
}

/// Binary infix operator.
///
/// Comparison and logical operators yield a dimensionless `0.0` or
/// `1.0` so they can be composed with arithmetic in the same
/// expression (e.g. `(a > b) * 5mm`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic.
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    // Comparison — yield Dimensionless 0.0 or 1.0.
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical — operate on Dimensionless 0/1.
    And,
    Or,
}

/// Prefix unary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    /// Numeric negation (`-x`).
    Neg,
    /// Logical not (`!x`).
    Not,
}

/// Array index variable used inside per-instance expressions.
///
/// `I` is the row index, `J` is the column index of a 2D array. For
/// 1D arrays only `I` is meaningful.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayIndex {
    I,
    J,
}

impl ExprNode {
    /// Construct a length literal in millimetres.
    pub fn literal_mm(value: f64) -> Self {
        Self::Literal(Quantity::length(value))
    }

    /// Construct a dimensionless count literal.
    pub fn literal_count(value: f64) -> Self {
        Self::Literal(Quantity::count(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binop_eq_compare() {
        assert_eq!(BinOp::Add, BinOp::Add);
        assert_ne!(BinOp::Add, BinOp::Sub);
    }

    #[test]
    fn unary_eq_compare() {
        assert_eq!(UnaryOp::Neg, UnaryOp::Neg);
        assert_ne!(UnaryOp::Neg, UnaryOp::Not);
    }

    #[test]
    fn expr_node_clone_roundtrip() {
        // a + 1 * 2
        let e = ExprNode::Binary(
            BinOp::Add,
            Box::new(ExprNode::Ref("a".to_string())),
            Box::new(ExprNode::Binary(
                BinOp::Mul,
                Box::new(ExprNode::literal_count(1.0)),
                Box::new(ExprNode::literal_count(2.0)),
            )),
        );
        let cloned = e.clone();
        assert_eq!(cloned, e);
    }

    #[test]
    fn tree_shape_assertions() {
        let tree = ExprNode::Binary(
            BinOp::Add,
            Box::new(ExprNode::Ref("a".to_string())),
            Box::new(ExprNode::Literal(Quantity::count(1.0))),
        );

        match tree {
            ExprNode::Binary(op, lhs, rhs) => {
                assert_eq!(op, BinOp::Add);
                match *lhs {
                    ExprNode::Ref(name) => assert_eq!(name, "a"),
                    other => panic!("expected Ref, got {other:?}"),
                }
                match *rhs {
                    ExprNode::Literal(q) => assert_eq!(q, Quantity::count(1.0)),
                    other => panic!("expected Literal, got {other:?}"),
                }
            }
            other => panic!("expected Binary, got {other:?}"),
        }
    }
}
