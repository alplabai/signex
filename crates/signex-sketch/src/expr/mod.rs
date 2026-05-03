//! Expression language for the sketch parameter table.
//!
//! Parameters can carry expressions — `body_w = "= pad_pitch *
//! (pin_count - 1) + 2.0mm"` — which are parsed into the
//! [`ast::ExprNode`] tree by [`parse::parse`] and evaluated to a
//! canonical-unit `f64` by [`eval::eval`]. The evaluator carries
//! [`unit::Quantity`] values throughout and unit-checks every
//! binary op so a `mm + deg` expression fails fast.
//!
//! The persisted on-disk form is the source `String`; the AST is
//! rebuilt on load by re-parsing.

pub mod ast;
pub mod eval;
pub mod parse;

use thiserror::Error;

use crate::unit::{Unit, UnitError, UnitFamily};

/// Errors produced by the expression layer (parse + evaluate +
/// parameter resolution).
#[derive(Clone, Debug, PartialEq, Error)]
pub enum ExprError {
    /// Parser found a syntactically invalid expression.
    #[error("expression parse error at byte {pos}: {msg}")]
    Parse { pos: usize, msg: String },

    /// Parser saw an unknown unit suffix (delegated from `unit::parse_quantity`).
    #[error("unknown or malformed quantity: {0}")]
    BadQuantity(String),

    /// Evaluator looked up a parameter that's not in the table.
    #[error("unknown parameter '{0}'")]
    Unknown(String),

    /// Evaluator hit a binary op whose operands have incompatible
    /// unit families (e.g. `1mm + 90deg`).
    #[error("unit mismatch in binary op: {lhs:?} vs {rhs:?}")]
    UnitMismatch { lhs: Unit, rhs: Unit },

    /// Evaluator called a function on a value of the wrong unit family.
    #[error("wrong unit family: expected {expected:?}, got {got:?}")]
    WrongFamily {
        expected: UnitFamily,
        got: UnitFamily,
    },

    /// Evaluator hit `ExprNode::ArrayIndex(..)` outside an array
    /// expansion context.
    #[error("`i` or `j` used outside an array expansion")]
    ArrayIndexOutsideArray,

    /// Evaluator hit a `Lookup` whose key doesn't match any of the
    /// keys list.
    #[error("lookup key did not match any of the provided keys")]
    LookupNoMatch,

    /// Evaluator hit a `Lookup` whose `keys.len() != values.len()`.
    #[error("lookup keys and values have different lengths")]
    LookupShapeMismatch,

    /// Parameter table contains a dependency cycle (a → b → a).
    #[error("parameter cycle detected involving '{0}'")]
    Cycle(String),

    /// Domain error in a function — `0^0`, `log(0)`, etc.
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
