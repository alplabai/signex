//! The sketch's view of the shared expression engine (`signex-expr`).
//!
//! Parameters can carry expressions — `body_w = "= pad_pitch *
//! (pin_count - 1) + 2.0mm"` — parsed by [`parse::parse`] and evaluated
//! by [`eval::eval`]. The engine (parser, AST, unit algebra, caps) lives
//! in `signex-expr`; what stays here is sketch-only:
//!
//! - the array index `i` / `j` ([`ast::ArrayIndex`]), resolved by
//!   [`eval::EvalContext`];
//! - "a bare number is mm" ([`eval::EvalContext::with_unitless_as_mm`]);
//! - the boundary check that a sketch value is a length, an angle or a
//!   count ([`eval::sketch_value`]), so an area such as `2mm * 3mm` is
//!   refused where the sketch reads it.
//!
//! The persisted on-disk form is the source `String`; the AST is
//! rebuilt on load by re-parsing.

pub mod ast;
pub mod eval;
pub mod parse;

pub use signex_expr::{ExprError, MAX_EXPR_CHAIN, MAX_EXPR_DEPTH, MAX_EXPR_LEN};
