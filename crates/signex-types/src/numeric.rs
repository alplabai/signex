//! Finite-number parsing for user-typed and file-borne text.
//!
//! `f64::from_str` accepts `NaN`, `inf`, `infinity` and overflows `1e999`
//! to infinity. A non-finite value stored as geometry makes later maths
//! produce NaN coordinates, so every boundary that reads a number from
//! text goes through [`parse_finite_f64`] instead.

use thiserror::Error;

/// Why [`parse_finite_f64`] refused a piece of text.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum FiniteParseError {
    /// The text is not a number at all (`1,5`, `1.5mm`, a lone `-`).
    #[error("{0:?} is not a number")]
    NotANumber(String),
    /// The text names `NaN` or an infinity, or overflows `f64`.
    #[error("{0:?} is not a finite number")]
    NonFinite(String),
}

/// Parse `text` (surrounding whitespace ignored) as a finite `f64`.
pub fn parse_finite_f64(text: &str) -> Result<f64, FiniteParseError> {
    let trimmed = text.trim();
    match trimmed.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        Ok(_) => Err(FiniteParseError::NonFinite(trimmed.to_string())),
        Err(_) => Err(FiniteParseError::NotANumber(trimmed.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_finite_numbers() {
        assert_eq!(parse_finite_f64(" 1.5 "), Ok(1.5));
        assert_eq!(parse_finite_f64("-2"), Ok(-2.0));
        assert_eq!(parse_finite_f64("1e3"), Ok(1000.0));
    }

    #[test]
    fn rejects_non_finite_text() {
        for bad in ["nan", "NaN", "inf", "-inf", "infinity", "1e999", "-1e999"] {
            assert!(
                matches!(parse_finite_f64(bad), Err(FiniteParseError::NonFinite(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn rejects_text_that_is_not_a_number() {
        for bad in ["", "-", "1,5", "1.5mm", "abc"] {
            assert!(
                matches!(parse_finite_f64(bad), Err(FiniteParseError::NotANumber(_))),
                "{bad:?}"
            );
        }
    }
}
