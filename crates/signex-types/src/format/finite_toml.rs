//! Save-time guard for the f64 values carried in the `[extras.*]` TOML
//! tables (#828).
//!
//! The TSV rows refuse a non-finite number through `mm_to_nm` /
//! `format_f64`. The extras tables are written by serde instead, and TOML
//! has `nan` / `inf` literals, so a NaN text size or an infinite outline
//! point would save without complaint and come back as the same poison.
//! The writer runs this over the extras tree first and fails the save,
//! naming the offending key path, before any byte reaches the file.

use serde::Serialize;

use super::FormatError;

/// Refuse `tree` if any float in it is NaN or infinite.
pub(in crate::format) fn ensure_finite<T: Serialize>(tree: &T) -> Result<(), FormatError> {
    let value = toml::Value::try_from(tree)?;
    let mut path = String::new();
    match first_non_finite(&value, &mut path) {
        Some(path) => Err(FormatError::NonFiniteExtra { path }),
        None => Ok(()),
    }
}

/// Depth-first search; `path` is the dotted key path of `value`, reused
/// across the walk so a hit can report where it was found.
fn first_non_finite(value: &toml::Value, path: &mut String) -> Option<String> {
    match value {
        toml::Value::Float(f) if !f.is_finite() => Some(path.clone()),
        toml::Value::Table(table) => table.iter().find_map(|(key, child)| {
            let mark = path.len();
            if !path.is_empty() {
                path.push('.');
            }
            path.push_str(key);
            let hit = first_non_finite(child, path);
            path.truncate(mark);
            hit
        }),
        toml::Value::Array(items) => items.iter().enumerate().find_map(|(i, child)| {
            let mark = path.len();
            path.push_str(&format!("[{i}]"));
            let hit = first_non_finite(child, path);
            path.truncate(mark);
            hit
        }),
        _ => None,
    }
}
