//! Expression AST.
//!
//! The AST is constructed by the parser (`crate::parse`) from a value's
//! source `String`. The AST itself is *not* serialised; the persisted form
//! is the source string, and the AST is rebuilt by re-parsing on load.
//! This keeps the on-disk format simple and human-editable.
//!
//! # Design
//!
//! Standard pure-functional expression tree:
//!
//! - [`ExprNode::Literal`] — a [`Quantity`] (value + unit)
//! - [`ExprNode::Ref`] — a name the evaluator looks up through a
//!   [`crate::Resolver`] (`pad_pitch`, `R1.Value`, a sketch array's `i`)
//! - [`ExprNode::Binary`] — infix binary operators (arithmetic,
//!   comparison, logical)
//! - [`ExprNode::Unary`] — prefix unary operators (negate, logical not)
//! - [`ExprNode::Ternary`] — `cond ? then : else`
//! - [`ExprNode::Lookup`] — table lookup, used for parametric pad-shape
//!   selection by package family
//! - [`ExprNode::Call`] — a pure function (`min`, `sqrt`, `round`, ...)

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
    /// Reference to a name (e.g. `pad_pitch`, `R1.Value`).
    Ref(String),
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
    /// A function call. The parser checks the argument count with
    /// [`Func::accepts`]; the evaluator checks it again for hand-built trees.
    Call(Func, Vec<ExprNode>),
}

/// The pure functions an expression may call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Func {
    /// Smallest of two or more values of one dimension.
    Min,
    /// Largest of two or more values of one dimension.
    Max,
    /// Absolute value, unit kept.
    Abs,
    /// Square root; a unit needs even exponents (`sqrt(4mm*mm)` is `2mm`).
    Sqrt,
    /// `round(x)` for a plain number, `round(x, step)` with a step of
    /// x's dimension (`round(R, 1k)`).
    Round,
    /// Like [`Func::Round`], toward negative infinity.
    Floor,
    /// Like [`Func::Round`], toward positive infinity.
    Ceil,
}

impl Func {
    /// Every function, for name lookup.
    pub const ALL: [Func; 7] = [
        Func::Min,
        Func::Max,
        Func::Abs,
        Func::Sqrt,
        Func::Round,
        Func::Floor,
        Func::Ceil,
    ];

    /// The name the function is called by.
    pub const fn name(self) -> &'static str {
        match self {
            Func::Min => "min",
            Func::Max => "max",
            Func::Abs => "abs",
            Func::Sqrt => "sqrt",
            Func::Round => "round",
            Func::Floor => "floor",
            Func::Ceil => "ceil",
        }
    }

    /// The function called `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }

    /// Fewest and most arguments; `None` means no upper bound.
    pub const fn arity(self) -> (usize, Option<usize>) {
        match self {
            Func::Min | Func::Max => (2, None),
            Func::Abs | Func::Sqrt => (1, Some(1)),
            Func::Round | Func::Floor | Func::Ceil => (1, Some(2)),
        }
    }

    /// True when `count` arguments are accepted.
    pub fn accepts(self, count: usize) -> bool {
        let (fewest, most) = self.arity();
        count >= fewest && most.is_none_or(|m| count <= m)
    }
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
