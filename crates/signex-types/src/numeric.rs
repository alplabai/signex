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

/// Append `key` to `path`, quoted TOML-style unless it is a bare key.
fn push_key(path: &mut String, key: &str) {
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare {
        path.push_str(key);
    } else {
        path.push('"');
        for c in key.chars() {
            match c {
                '"' | '\\' => {
                    path.push('\\');
                    path.push(c);
                }
                '\n' => path.push_str("\\n"),
                '\t' => path.push_str("\\t"),
                '\r' => path.push_str("\\r"),
                c if c.is_control() => path.push_str(&format!("\\u{:04X}", c as u32)),
                c => path.push(c),
            }
        }
        path.push('"');
    }
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
                push_key(path, k);
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
    // `Value::try_into` has no spans, so on a schema error re-parse straight
    // into `T` to get the error with its line/column. Error path only.
    value
        .try_into()
        .or_else(|_| toml::from_str::<T>(text))
        .map_err(TomlFiniteError::Toml)
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
    Ok(first_non_finite_toml_path(&value))
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

    #[derive(Debug, serde::Deserialize)]
    struct Probe {
        #[allow(dead_code)]
        needed: f64,
    }

    /// Schema errors must keep their line/column (a span-less
    /// `Value::try_into` loses them).
    #[test]
    fn schema_error_keeps_line_number() {
        let err = parse_toml_finite::<Probe>(
            "other = 1
",
        )
        .unwrap_err();
        assert!(err.to_string().contains("line 1"), "{err}");
        let err = parse_toml_finite::<Probe>(
            "

needed = \"x\"
",
        )
        .unwrap_err();
        assert!(err.to_string().contains("line 3"), "{err}");
    }

    #[derive(Debug, serde::Deserialize)]
    struct Typed {
        #[allow(dead_code)]
        x: f64,
        #[allow(dead_code)]
        pts: Vec<Typed2>,
    }
    #[derive(Debug, serde::Deserialize)]
    struct Typed2 {
        #[allow(dead_code)]
        y: f64,
    }

    fn path_of(text: &str) -> String {
        match parse_toml_finite::<toml::Value>(text) {
            Err(TomlFiniteError::NonFinite { path }) => path,
            other => panic!("expected NonFinite, got {other:?}"),
        }
    }

    #[test]
    fn nan_in_typed_field_is_refused() {
        let err = parse_toml_finite::<Typed>(
            "x = nan
pts = []
",
        )
        .unwrap_err();
        assert!(matches!(err, TomlFiniteError::NonFinite { ref path } if path == "x"));
    }

    #[test]
    fn nan_in_array_of_tables_names_index() {
        let text = "x = 1.0
[[pts]]
y = 1.0
[[pts]]
y = inf
";
        let err = parse_toml_finite::<Typed>(text).unwrap_err();
        assert!(matches!(err, TomlFiniteError::NonFinite { ref path } if path == "pts[1].y"));
    }

    /// The non-finite check runs before the schema check, so a file with
    /// both reports the non-finite value first.
    #[test]
    fn non_finite_is_reported_before_schema_error() {
        let err = parse_toml_finite::<Probe>("x = nan\nneeded = \"s\"\n").unwrap_err();
        assert!(matches!(err, TomlFiniteError::NonFinite { ref path } if path == "x"));
    }

    #[test]
    fn control_chars_in_keys_are_escaped() {
        assert_eq!(path_of("[\"a\\nb\"]\nv = nan\n"), "\"a\\nb\".v");
        assert_eq!(path_of("[\"a\\tb\"]\nv = nan\n"), "\"a\\tb\".v");
        assert_eq!(path_of("[\"a\\u0001b\"]\nv = nan\n"), "\"a\\u0001b\".v");
    }

    #[test]
    fn awkward_keys_are_quoted_toml_style() {
        assert_eq!(
            path_of(
                "[\"a.b\"]
v = nan
"
            ),
            "\"a.b\".v"
        );
        assert_eq!(
            path_of(
                "[\"a b\"]
v = nan
"
            ),
            "\"a b\".v"
        );
        assert_eq!(
            path_of(
                "[\"q\\\"r\"]
v = nan
"
            ),
            "\"q\\\"r\".v"
        );
        assert_eq!(
            path_of(
                "[ok-1_x]
v = nan
"
            ),
            "ok-1_x.v"
        );
    }
}
