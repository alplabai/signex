//! Constraint types live in [`signex_types::sketch::constraint`]; this
//! module re-exports them and keeps the dimension-input parser, which
//! belongs to the expression language.

pub use signex_types::sketch::constraint::*;

/// Parse the text of a dimension input box into a canonical-unit number.
///
/// Rejects anything that is not a finite number: `NaN`, `inf` and values
/// that overflow `f64` all parse under `f64::from_str` but would make
/// every later solve diverge.
pub fn parse_dimension_input(text: &str) -> Result<f64, crate::expr::ExprError> {
    signex_types::numeric::parse_finite_f64(text)
        .map_err(|_| crate::expr::ExprError::BadQuantity(text.to_string()))
}
