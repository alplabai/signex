//! Expression AST: the shared tree from `signex-expr` plus the sketch's
//! array index.

pub use signex_expr::ast::{BinOp, ExprNode, Func, UnaryOp};

/// Array index variable used inside per-instance expressions.
///
/// `I` is the row index, `J` is the column index of a 2D array. For
/// 1D arrays only `I` is meaningful. In the tree an index is the
/// reference `i` or `j`; [`crate::expr::eval::EvalContext`] binds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayIndex {
    I,
    J,
}

impl ArrayIndex {
    /// The name the index is written as.
    pub const fn name(self) -> &'static str {
        match self {
            ArrayIndex::I => "i",
            ArrayIndex::J => "j",
        }
    }

    /// The index called `name`. Only the exact one-letter names count:
    /// `ix`, `jj` and `ij` are ordinary references.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "i" => Some(ArrayIndex::I),
            "j" => Some(ArrayIndex::J),
            _ => None,
        }
    }

    /// The tree node for this index.
    pub fn node(self) -> ExprNode {
        ExprNode::Ref(self.name().to_string())
    }

    /// The index `node` is, if it is one.
    pub fn of(node: &ExprNode) -> Option<Self> {
        match node {
            ExprNode::Ref(name) => Self::from_name(name),
            _ => None,
        }
    }
}
