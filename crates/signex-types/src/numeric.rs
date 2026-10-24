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

/// Key path (`extras.board.outline[0].x`) of the first NaN or infinite
/// float in `tree`, or `None` when every float is finite.
///
/// TOML spells those values `nan` / `inf`, so a serde-written table
/// carries them without complaint and a save would persist the poison.
/// Writers run this over what they are about to serialise and refuse the
/// save, naming the key, before any byte is written.
pub fn first_non_finite_path<T: serde::Serialize>(
    tree: &T,
) -> Result<Option<String>, toml::ser::Error> {
    let value = toml::Value::try_from(tree)?;
    Ok(non_finite_in(&value, &mut String::new()))
}

/// Depth-first search; `path` is the key path of `value`, reused across
/// the walk so a hit can report where it was found.
fn non_finite_in(value: &toml::Value, path: &mut String) -> Option<String> {
    match value {
        toml::Value::Float(f) if !f.is_finite() => Some(path.clone()),
        toml::Value::Table(table) => table.iter().find_map(|(key, child)| {
            let mark = path.len();
            if !path.is_empty() {
                path.push('.');
            }
            path.push_str(key);
            let hit = non_finite_in(child, path);
            path.truncate(mark);
            hit
        }),
        toml::Value::Array(items) => items.iter().enumerate().find_map(|(i, child)| {
            let mark = path.len();
            path.push_str(&format!("[{i}]"));
            let hit = non_finite_in(child, path);
            path.truncate(mark);
            hit
        }),
        _ => None,
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
