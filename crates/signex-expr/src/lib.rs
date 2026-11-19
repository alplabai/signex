//! The expression engine shared by every Signex value that may be a formula:
//! footprint sketch parameters today, part fields, sheet and project
//! variables, BOM columns and board rules next (#1984).
//!
//! A value is stored as its source text. A leading `=` marks an expression
//! and `==` escapes a literal leading `=` ([`source::classify`]); the source
//! string is what is saved, never the evaluated result, and the tree is
//! rebuilt by re-parsing on load.
//!
//! - [`parse::parse`] turns source text into an [`ast::ExprNode`] tree.
//! - [`eval::eval`] reduces a tree to a [`unit::Quantity`]. Every operator
//!   is unit-checked and `*`, `/`, `^` and `sqrt` derive the result's unit
//!   from a [`dimension::Dimension`] vector, so `3.3V / 10mA` is `330Ω`.
//! - A name in an expression is looked up through a [`resolver::Resolver`],
//!   so each host (a sketch's parameter table, a part's fields, sheet and
//!   project variables) supplies its own names. A reference cycle is an
//!   error, never a stack overflow.
//!
//! No UI, no IO and no workspace dependencies: see
//! `docs/architecture/signex-expr.md` for where the crate sits.

pub mod ast;
pub mod deps;
pub mod dimension;
pub mod eval;
mod format;
mod func;
mod lex;
pub mod parse;
pub mod resolver;
pub mod source;
pub mod symbols;
pub mod unit;

use thiserror::Error;

pub use ast::{BinOp, ExprNode, Func, UnaryOp};
pub use deps::{dependency_order, references};
pub use dimension::{Dimension, MAX_DIM_EXPONENT};
pub use eval::eval;
pub use parse::{RESERVED_CONSTANTS, is_reserved_constant, parse, parse_with};
pub use resolver::{Binding, Layered, NoBindings, Resolver};
pub use source::{FieldText, classify, expression_body};
pub use symbols::{BareM, LiteralContext, bare_m_meaning};
pub use unit::{Quantity, Unit, UnitError, UnitFamily, parse_quantity, parse_quantity_with};

/// Deepest expression nesting the parser accepts (parenthesis groups plus
/// chained unary operators). Real parameter expressions nest a handful of
/// levels; the cap keeps the recursive-descent parser, the evaluator and
/// the recursive `Drop` of the AST well inside the thread stack. It also
/// bounds how many references one evaluation may follow in a chain.
pub const MAX_EXPR_DEPTH: usize = 64;

/// Most left-associative binary operators one expression may hold. A flat
/// chain such as `1+1+1+...` nests one tree level per operator without any
/// parser recursion, so this count (not the depth cap) bounds the
/// evaluator's and `Drop`'s recursion for those trees.
pub const MAX_EXPR_CHAIN: usize = 256;

/// Deepest tree the evaluator walks in one evaluation, counted across every
/// reference it follows. An expression at the chain cap that names a body at
/// the chain cap still evaluates; a longer chain of such bodies runs out of
/// budget with [`ExprError::TooDeep`] instead of overflowing the stack. The
/// budget is sized to fit a 1 MiB thread stack in a debug build (the Windows
/// main thread); twice the full tree depth did not.
pub const MAX_EVAL_DEPTH: usize = 2 * MAX_EXPR_CHAIN + MAX_EXPR_DEPTH / 2;

/// Longest expression source, in bytes, the parser accepts.
pub const MAX_EXPR_LEN: usize = 4096;

/// Errors produced by the expression layer (parse + evaluate + name
/// resolution).
#[derive(Clone, Debug, PartialEq, Error)]
pub enum ExprError {
    /// Parser found a syntactically invalid expression.
    #[error("expression parse error at byte {pos}: {msg}")]
    Parse { pos: usize, msg: String },

    /// Parser saw an unknown unit suffix (delegated from `unit::parse_quantity`).
    #[error("unknown or malformed quantity: {0}")]
    BadQuantity(String),

    /// Evaluator looked up a name no resolver knows.
    #[error("unknown parameter '{0}'")]
    Unknown(String),

    /// Evaluator hit an operator whose operands have incompatible units
    /// (e.g. `1mm + 90deg`, `1V + 1A`).
    #[error("unit mismatch in binary op: {lhs:?} vs {rhs:?}")]
    UnitMismatch { lhs: unit::Unit, rhs: unit::Unit },

    /// Evaluator needed a value of one unit family and got another.
    #[error("wrong unit family: expected {expected:?}, got {got:?}")]
    WrongFamily {
        expected: unit::UnitFamily,
        got: unit::UnitFamily,
    },

    /// A host's index variable (`i` / `j` of a sketch array) was used
    /// outside the scope that binds it. Raised by that host's resolver.
    #[error("`i` or `j` used outside an array expansion")]
    ArrayIndexOutsideArray,

    /// Evaluator hit a `Lookup` whose key doesn't match any of the
    /// keys list.
    #[error("lookup key did not match any of the provided keys")]
    LookupNoMatch,

    /// Evaluator hit a `Lookup` whose `keys.len() != values.len()`.
    #[error("lookup keys and values have different lengths")]
    LookupShapeMismatch,

    /// A host tried to bind a value under a reserved constant's name
    /// ([`RESERVED_CONSTANTS`]); every expression would read the constant.
    #[error("'{0}' is a reserved constant and cannot be a parameter name")]
    ReservedName(String),

    /// Names refer to each other in a cycle (a → b → a).
    #[error("parameter cycle detected involving '{0}'")]
    Cycle(String),

    /// An evaluated value, or a numeric literal, is NaN or infinite.
    #[error("non-finite number: {0}")]
    NonFinite(String),

    /// Expression nesting exceeds [`MAX_EXPR_DEPTH`], a reference chain is
    /// longer than [`MAX_EXPR_DEPTH`], or one evaluation, across the
    /// references it follows, walks deeper than [`MAX_EVAL_DEPTH`].
    #[error("expression nests deeper than {max} levels")]
    TooDeep { max: usize },

    /// One expression chains more than [`MAX_EXPR_CHAIN`] binary
    /// operators (`1+1+1+...`). Not a nesting problem: the chain is flat.
    #[error("expression chains more than {max} operators; the chain is too long")]
    ChainTooLong { max: usize },

    /// Expression source is longer than [`MAX_EXPR_LEN`] bytes.
    #[error("expression is {len} bytes, over the limit of {max}")]
    TooLong { max: usize, len: usize },

    /// Domain error in an operator or function — division by zero,
    /// `sqrt` of a negative number, a unit exponent out of range.
    #[error("domain error: {0}")]
    Domain(String),
}

impl From<UnitError> for ExprError {
    fn from(e: UnitError) -> Self {
        match e {
            UnitError::Parse(s) => ExprError::BadQuantity(s),
            UnitError::WrongFamily { expected, got } => ExprError::WrongFamily { expected, got },
            UnitError::Incompatible(lhs, rhs) => ExprError::UnitMismatch { lhs, rhs },
        }
    }
}
