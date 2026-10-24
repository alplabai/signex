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

/// Why [`parse_toml_finite`] refused a document.
#[derive(Debug, Error)]
pub enum TomlFiniteError {
    /// The text is not valid TOML for the target type.
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    /// TOML accepts `nan` / `inf`; a document carrying one could be opened
    /// but never saved (writers refuse non-finite numbers), so loading it
    /// is refused. `path` names the offending key (`extras.pcb.keepout[2].x`).
    #[error("non-finite number (nan/inf) at `{path}`")]
    NonFinite { path: String },
}

/// Key path of the first `nan` / `inf` float in `value`, if any.
pub fn first_non_finite_toml_path(value: &toml::Value) -> Option<String> {
    fn walk(v: &toml::Value, path: &mut String) -> bool {
        match v {
            toml::Value::Float(f) => !f.is_finite(),
            toml::Value::Array(items) => items.iter().enumerate().any(|(i, item)| {
                let len = path.len();
                path.push_str(&format!("[{i}]"));
                walk(item, path) || {
                    path.truncate(len);
                    false
                }
            }),
            toml::Value::Table(table) => table.iter().any(|(k, item)| {
                let len = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(k);
                walk(item, path) || {
                    path.truncate(len);
                    false
                }
            }),
            _ => false,
        }
    }
    let mut path = String::new();
    walk(value, &mut path).then_some(path)
}

/// `toml::from_str`, but a `nan` / `inf` anywhere in the document is an
/// error naming its key path instead of a file that cannot be saved.
pub fn parse_toml_finite<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, TomlFiniteError> {
    let value: toml::Value = toml::from_str(text)?;
    if let Some(path) = first_non_finite_toml_path(&value) {
        return Err(TomlFiniteError::NonFinite { path });
    }
    Ok(value.try_into()?)
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
