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
    parse_named_field_number("Numeric field edit", text)
}

/// [`parse_field_number`] for a dialog box the refusal line should name
/// (`"Move Selection X"`), so the user can tell which box to fix.
pub(crate) fn parse_named_field_number(field: &str, text: &str) -> FieldNumber {
    match parse_finite_f64(text) {
        Ok(v) => FieldNumber::Number(v),
        Err(FiniteParseError::NotANumber(_)) => FieldNumber::NotANumber,
        Err(err @ FiniteParseError::NonFinite(_)) => {
            crate::diagnostics::log_warning(format!("{field} refused: {err}"));
            FieldNumber::Refused
        }
    }
}

/// Parse a grid step field (Grid Properties Step, the fast grids): it must
/// be a finite number above zero. The error is the user-facing reason for
/// the refusal.
pub(crate) fn validated_grid_step(text: &str) -> Result<f64, String> {
    match parse_finite_f64(text) {
        Ok(step) => positive_grid_step(step),
        Err(err) => Err(err.to_string()),
    }
}

/// A grid step that is already a number: finite and above zero, or the
/// reason it is refused.
pub(crate) fn positive_grid_step(step: f64) -> Result<f64, String> {
    if !step.is_finite() {
        Err(format!("{step} is not a finite number"))
    } else if step > 0.0 {
        Ok(step)
    } else {
        Err(format!("{step} is not above zero"))
    }
}

/// [`positive_grid_step`] for a grid held as `f32` mm (the schematic snap
/// grid): also refused when it does not survive the narrowing (overflows to
/// infinity or underflows to zero).
pub(crate) fn positive_grid_step_f32(step: f64) -> Result<f32, String> {
    let mm = positive_grid_step(step)? as f32;
    if mm.is_finite() && mm > 0.0 {
        Ok(mm)
    } else {
        Err(format!("{step} is out of range for a grid"))
    }
}

#[cfg(test)]
mod grid_step_tests {
    use super::*;

    #[test]
    fn a_grid_step_must_be_finite_and_above_zero() {
        assert_eq!(validated_grid_step(" 1.27 "), Ok(1.27));
        assert!(validated_grid_step("0").is_err());
        assert!(validated_grid_step("-2.54").is_err());
        assert!(validated_grid_step("nan").is_err());
        assert!(validated_grid_step("inf").is_err());
        assert!(validated_grid_step("1.27mm").is_err());
    }

    #[test]
    fn an_f32_grid_step_refuses_what_the_narrowing_breaks() {
        assert_eq!(positive_grid_step_f32(2.54), Ok(2.54_f32));
        assert!(positive_grid_step_f32(1e300).is_err());
        assert!(positive_grid_step_f32(1e-300).is_err());
        assert!(positive_grid_step_f32(f64::NAN).is_err());
    }
}
