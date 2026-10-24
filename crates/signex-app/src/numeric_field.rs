//! Numeric text-field parsing for the editors. The domain rule lives in
//! [`signex_types::numeric`]; this adds the part the app owns, telling
//! the user (Messages panel) when a value was refused.

use signex_types::numeric::{FiniteParseError, parse_finite_f64};

/// Parse a typed numeric field. `None` means "leave the stored value
/// alone": the text is mid-edit (`1.`, `-`) or not a finite number.
///
/// Text that is *not a number at all* stays quiet because the user is
/// usually still typing it. `nan` / `inf` / an overflow is a complete,
/// deliberate-looking input that would corrupt geometry, so it is
/// reported at `warn!` (the default log filter drops `debug!`).
pub(crate) fn parse_field_f64(text: &str) -> Option<f64> {
    match parse_finite_f64(text) {
        Ok(v) => Some(v),
        Err(FiniteParseError::NotANumber(_)) => None,
        Err(err @ FiniteParseError::NonFinite(_)) => {
            crate::diagnostics::log_warning(format!("Numeric field edit refused: {err}"));
            None
        }
    }
}
