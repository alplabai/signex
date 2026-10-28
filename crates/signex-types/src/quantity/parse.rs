//! Reading stored unit strings and the number field of an editor.
//!
//! There is deliberately no free-text `10k` / `4k7` / `4.7uF` parser:
//! editors take a plain number plus an SI-prefix picker and a unit picker.
//! Values are assembled by joining decimal strings (`"4.7"` + `k` ->
//! `"4.7e3"`) and parsing once, never by multiplying floats, so `4.7`+k and
//! `4700`+none are the same double.

use thiserror::Error;

use super::prefix::SiPrefix;
use super::unit::Unit;
use super::value::Quantity;
use crate::numeric::{FiniteParseError, parse_finite_f64};

/// Why the number field or a unit/prefix pairing was refused.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum MantissaError {
    #[error("enter a number")]
    Empty,
    /// Letters after (or in place of) the number, e.g. `10k`, `4.7uF`.
    /// The prefix and unit are chosen with the pickers, not typed.
    #[error("{0:?} contains letters; type only the number and pick the prefix and unit")]
    HasLetters(String),
    /// `4,7` is refused rather than guessed (thousands separator?).
    #[error("{0:?} uses a comma; use a point as decimal separator")]
    Comma(String),
    #[error("{0:?} is not a number")]
    NotANumber(String),
    #[error("{0:?} is not a finite number")]
    NonFinite(String),
    #[error("{0} does not take an SI prefix")]
    PrefixNotAllowed(String),
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

/// `"<mantissa>e<exp>"`: the mantissa text with `add` added to its own
/// exponent (if any). The text must already be a valid plain number.
fn join_exponent(mantissa: &str, add: i32) -> String {
    let text = mantissa.trim();
    match text.find(['e', 'E']) {
        Some(i) => {
            let own: i32 = text[i + 1..].parse().unwrap_or(0);
            format!("{}e{}", &text[..i], own + add)
        }
        None => format!("{text}e{add}"),
    }
}

/// The base value of `mantissa` (number-field text) times `prefix`,
/// computed by decimal string join and one correctly rounded parse.
pub fn quantity_from_parts(
    mantissa: &str,
    prefix: SiPrefix,
    unit: Unit,
) -> Result<Quantity, MantissaError> {
    parse_mantissa(mantissa)?;
    if prefix != SiPrefix::One && !unit.takes_prefix() {
        return Err(MantissaError::PrefixNotAllowed(unit.token().to_string()));
    }
    let joined = join_exponent(mantissa, prefix.exponent());
    let base: f64 = joined
        .parse()
        .map_err(|_| MantissaError::NotANumber(mantissa.trim().to_string()))?;
    Quantity::new(base, unit).ok_or_else(|| MantissaError::NonFinite(mantissa.trim().to_string()))
}

impl Quantity {
    /// [`quantity_from_parts`] for an `f64` mantissa such as a stored
    /// `Measurement.value`. `None` if the result is not finite.
    pub fn from_mantissa(mantissa: f64, prefix: SiPrefix, unit: Unit) -> Option<Quantity> {
        if !mantissa.is_finite() {
            return None;
        }
        // `{:e}` is the shortest round-trip form, so this is exact.
        let joined = join_exponent(&format!("{mantissa:e}"), prefix.exponent());
        Quantity::new(joined.parse().ok()?, unit)
    }
}

/// Split a stored `Measurement.unit` string into the `(prefix, unit)` it
/// spells. The unit is matched whole first (`Hz`, `ppm/C`), then as a prefix
/// followed by a unit. Spaces inside are ignored (`k Ω`). `None` if it is not exactly
/// an optional prefix and a known unit, or if the unit takes no prefix.
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
/// quantity. An unrecognised unit string is kept as [`Unit::Other`] with
/// the value as written. `None` for a non-finite value.
pub fn quantity_from_stored(mantissa: f64, unit_string: &str) -> Option<Quantity> {
    match split_unit_string(unit_string) {
        Some((prefix, unit)) => Quantity::from_mantissa(mantissa, prefix, unit),
        None => Quantity::new(mantissa, Unit::from_token_lossy(unit_string)),
    }
}

/// Whether a stored unit string (`found`, possibly prefixed: `kΩ`) is the
/// same unit as a template's `expected` token (`ohm`). `kF` is not `ohm`.
/// Unrecognised strings match only when written identically.
pub fn unit_string_matches(found: &str, expected: &str) -> bool {
    if found == expected {
        return true;
    }
    match (split_unit_string(found), split_unit_string(expected)) {
        (Some((_, f)), Some((_, e))) => f == e,
        _ => false,
    }
}
