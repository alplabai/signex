//! What the copy of a newer-minor project would not keep (#2908). This build
//! reads some keys leniently (an unreadable value reads as absent) and some
//! objects with no bag for keys it does not know, so re-encoding such a
//! project can drop or change a key without an error. Before the copy is
//! written, the source file is read again and copied as this build reads it,
//! and every key of the source that this copy leaves out or changes is named.
//! The copy of the file on disk, not the one written: the written copy holds
//! the person's unsaved edits, and a key they changed is not a loss.

use std::path::Path;

use serde_json::{Map, Value};
use signex_types::project::parse_project_bytes;

use super::plan::copy_bytes;

/// Top-level keys the copy rewrites on purpose: the format token, its name,
/// its folder and its new id.
const REWRITTEN: [&str; 4] = ["format", "name", "dir", "document_id"];

/// Every key of `source` that `copy` leaves out or holds a different value
/// for, as a path (`fab_outputs.laser`, `sheets[1].colour`), sorted.
/// A key the copy adds is not a loss and is not named. Arrays of the same
/// length are compared item by item; otherwise the whole array is named.
pub(crate) fn dropped_keys(source: &Value, copy: &Value) -> Vec<String> {
    let mut out = Vec::new();
    // A project file is an object; anything else holds no key to keep.
    let empty = Map::new();
    let source = source.as_object().unwrap_or(&empty);
    let copy = copy.as_object().unwrap_or(&empty);
    for (key, value) in source {
        if REWRITTEN.contains(&key.as_str()) {
            continue;
        }
        compare(key, value, copy.get(key), &mut out);
    }
    out.sort();
    out
}

fn compare(path: &str, source: &Value, copy: Option<&Value>, out: &mut Vec<String>) {
    let Some(copy) = copy else {
        out.push(path.to_string());
        return;
    };
    match (source, copy) {
        (Value::Object(s), Value::Object(c)) => {
            for (key, value) in s {
                compare(&format!("{path}.{key}"), value, c.get(key), out);
            }
        }
        (Value::Array(s), Value::Array(c)) if s.len() == c.len() => {
            for (i, (value, other)) in s.iter().zip(c).enumerate() {
                compare(&format!("{path}[{i}]"), value, Some(other), out);
            }
        }
        _ if same(source, copy) => {}
        _ => out.push(path.to_string()),
    }
}

/// The same value; numbers compare by value, so `1` and `1.0` are the same.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// The keys of the `.snxprj` at `source_file` that its copy, named `name` in
/// `folder`, would not keep: the file as read is compared with the file as
/// this build reads and copies it. Run on the blocking pool: it reads the
/// source file.
pub(crate) fn dropped_from_file(
    source_file: &Path,
    name: &str,
    folder: &Path,
) -> Result<Vec<String>, String> {
    let read = std::fs::read(source_file)
        .map_err(|e| format!("could not read {}: {e}", source_file.display()))?;
    let text = std::str::from_utf8(&read)
        .map_err(|e| format!("{} is not UTF-8: {e}", source_file.display()))?;
    let source: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("could not read {}: {e}", source_file.display()))?;
    let as_read = parse_project_bytes(
        source_file,
        &read,
        signex_types::project::files_on_disk(source_file),
    )
    .map_err(|e| format!("could not read {}: {e}", source_file.display()))?;
    let bytes = copy_bytes(&as_read, name, folder)?;
    let copy: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(dropped_keys(&source, &copy))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::dropped_keys;

    #[test]
    fn a_key_left_out_or_changed_is_named_and_one_added_or_rewritten_is_not() {
        let source = json!({
            "format": "snxprj/1.9",
            "name": "P",
            "dir": "/a/P",
            "fab_outputs": {"gerber": false, "laser": true},
            "router_tool_diameter": "wide",
            "sheets": [{"filename": "a"}, {"filename": "b", "colour": 3}],
            "libraries": [1, 2],
            "kept": {"x": 1}
        });
        let copy = json!({
            "format": "snxprj/1",
            "name": "Copy",
            "dir": "/b/Copy",
            "fab_outputs": {"gerber": false, "drill": true},
            "sheets": [{"filename": "a"}, {"filename": "b"}],
            "libraries": [1],
            "kept": {"x": 1.0},
            "added": true
        });
        assert_eq!(
            dropped_keys(&source, &copy),
            [
                "fab_outputs.laser",
                "libraries",
                "router_tool_diameter",
                "sheets[1].colour",
            ]
        );
    }

    #[test]
    fn a_copy_that_keeps_everything_names_nothing() {
        let source = json!({"format": "snxprj/1.9", "a": {"b": [1, {"c": null}]}});
        let copy = json!({"format": "snxprj/1", "a": {"b": [1, {"c": null}]}});
        assert!(dropped_keys(&source, &copy).is_empty());
    }
}
