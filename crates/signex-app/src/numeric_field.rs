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
    match parse_field_number(text) {
        FieldNumber::Number(v) => Some(v),
        FieldNumber::NotANumber | FieldNumber::Refused => None,
    }
}

/// What a typed field's text is, for callers that treat "not a number"
/// (fall back to text) differently from "refused" (keep the old value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FieldNumber {
    /// A finite number.
    Number(f64),
    /// Not a number at all; the refusal is silent (the user may be mid-edit).
    NotANumber,
    /// `nan` / `inf` / an overflow. Already reported to the Messages panel.
    Refused,
}

/// Classify a typed numeric field, reporting a refusal at `warn!`.
pub(crate) fn parse_field_number(text: &str) -> FieldNumber {
    match parse_finite_f64(text) {
        Ok(v) => FieldNumber::Number(v),
        Err(FiniteParseError::NotANumber(_)) => FieldNumber::NotANumber,
        Err(err @ FiniteParseError::NonFinite(_)) => {
            crate::diagnostics::log_warning(format!("Numeric field edit refused: {err}"));
            FieldNumber::Refused
        }
    }
}
