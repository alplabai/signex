//! Reading stored unit strings and the number field of an editor.
//!
//! There is deliberately no free-text `10k` / `4k7` / `4.7uF` parser:
//! editors take a plain number plus an SI-prefix picker and a unit picker.

use thiserror::Error;

use super::unit::{SiPrefix, Unit};
use crate::numeric::{FiniteParseError, parse_finite_f64};

/// Why [`parse_mantissa`] refused the number field.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum MantissaError {
    #[error("enter a number")]
    Empty,
    /// Letters after (or in place of) the number, e.g. `10k`, `4.7uF`.
    /// The prefix and unit are chosen with the pickers, not typed.
    #[error("{0:?} contains letters; type only the number and pick the prefix and unit")]
    HasLetters(String),
    /// `4,7` is refused rather than guessed.
    #[error("{0:?} uses a comma; use a point as decimal separator")]
    Comma(String),
    #[error("{0:?} is not a number")]
    NotANumber(String),
    #[error("{0:?} is not a finite number")]
    NonFinite(String),
}

/// The editor's number field: a plain finite decimal (`4.7`, `-40`,
/// `1e3`), surrounding spaces ignored. Any suffix is an error.
pub fn parse_mantissa(input: &str) -> Result<f64, MantissaError> {
    if input.trim().is_empty() {
        return Err(MantissaError::Empty);
    }
    parse_finite_f64(input).map_err(|e| match e {
        FiniteParseError::NonFinite(t) => MantissaError::NonFinite(t),
        FiniteParseError::NotANumber(t) if t.contains(',') => MantissaError::Comma(t),
        FiniteParseError::NotANumber(t) if t.chars().any(char::is_alphabetic) => {
            MantissaError::HasLetters(t)
        }
        FiniteParseError::NotANumber(t) => MantissaError::NotANumber(t),
    })
}

/// Split a stored `Measurement.unit` string into the `(prefix, unit)` it
/// spells. The unit is matched whole first (`ppm`, `Hz`), then as prefix +
/// unit. Spaces inside are ignored (`k Ω`). `None` if it is not exactly an
/// optional prefix and a known unit, or if the unit takes no prefix.
pub fn split_unit_string(text: &str) -> Option<(SiPrefix, Unit)> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(unit) = Unit::from_alias(&compact) {
        return Some((SiPrefix::One, unit));
    }
    let mut chars = compact.chars();
    let prefix = SiPrefix::from_char(chars.next()?)?;
    let unit = Unit::from_alias(chars.as_str())?;
    unit.takes_prefix().then_some((prefix, unit))
}

/// `mantissa` of a stored `Measurement { value, unit }` as a base-unit
/// value, or `None` for an unrecognised unit string or a non-finite result.
pub fn base_value(mantissa: f64, unit_string: &str) -> Option<(f64, Unit)> {
    let (prefix, unit) = split_unit_string(unit_string)?;
    let base = prefix.scale(mantissa);
    base.is_finite().then_some((base, unit))
}
