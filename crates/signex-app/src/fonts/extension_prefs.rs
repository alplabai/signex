//! Extension preferences in `prefs.json` (#2495): one `extensions` object
//! that holds every extension's values under their full `ext.<name>.<local>`
//! keys, so they never meet a built-in preference.

use std::collections::BTreeMap;

use super::*;

/// The top-level `prefs.json` key that holds every extension preference.
pub const EXTENSION_PREFS_KEY: &str = "extensions";

/// The extension preferences stored in `json` (a whole `prefs.json`): the
/// entries of its `extensions` object whose key starts with `ext.`. Empty
/// when there is no such object.
pub fn read_extension_prefs_from(
    json: Option<&serde_json::Value>,
) -> BTreeMap<String, serde_json::Value> {
    json.and_then(|prefs| prefs.get(EXTENSION_PREFS_KEY))
        .and_then(serde_json::Value::as_object)
        .map(|stored| {
            stored
                .iter()
                .filter(|(key, _)| key.starts_with("ext."))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The extension preferences of the user's `prefs.json`. Blocking: read at
/// launch, before the first window.
pub fn read_extension_prefs() -> BTreeMap<String, serde_json::Value> {
    read_extension_prefs_from(load_prefs_json().as_ref())
}

/// Store `value` under `key` in the `extensions` object of the `prefs.json`
/// at `path`, keeping every other setting. Refused, with the file left as
/// it was, when the file cannot be read as preferences or its `extensions`
/// entry is not an object. Blocking: call it off the UI thread.
pub fn write_extension_pref_at(
    path: &std::path::Path,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let mut not_an_object = false;
    try_update_prefs_json(path, EXTENSION_PREFS_KEY, |prefs| {
        let stored = prefs
            .entry(EXTENSION_PREFS_KEY.to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        match stored.as_object_mut() {
            Some(stored) => {
                stored.insert(key.to_string(), value);
            }
            None => not_an_object = true,
        }
    })?;
    if not_an_object {
        return Err(format!(
            "{} holds an `{EXTENSION_PREFS_KEY}` entry that is not an object; it was left as it is ({key} not saved)",
            path.display()
        ));
    }
    Ok(())
}

/// [`write_extension_pref_at`] on the user's `prefs.json`.
pub fn write_extension_pref(key: &str, value: serde_json::Value) -> Result<(), String> {
    write_extension_pref_at(&prefs_path(), key, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extension_pref_is_saved_under_its_full_key_and_reads_back() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{"theme":"graphite_dark"}"#).expect("write prefs.json");

        write_extension_pref_at(&path, "ext.sample.greeting", serde_json::json!("hello"))
            .expect("saved");
        write_extension_pref_at(&path, "ext.other.count", serde_json::json!(3)).expect("saved");

        let json = read_prefs_json(&path).expect("prefs.json parses");
        assert_eq!(json["theme"], serde_json::json!("graphite_dark"));
        assert_eq!(
            json[EXTENSION_PREFS_KEY],
            serde_json::json!({"ext.sample.greeting": "hello", "ext.other.count": 3})
        );
        let stored = read_extension_prefs_from(Some(&json));
        assert_eq!(stored.len(), 2);
        assert_eq!(stored["ext.sample.greeting"], serde_json::json!("hello"));
    }

    /// Only keys in the extension namespace are read back.
    #[test]
    fn a_stored_key_outside_the_namespace_is_not_read() {
        let json = serde_json::json!({
            "extensions": {"ext.sample.a": 1, "theme": "x", "sample.b": 2}
        });
        let stored = read_extension_prefs_from(Some(&json));
        assert_eq!(stored.keys().collect::<Vec<_>>(), ["ext.sample.a"]);
        assert!(read_extension_prefs_from(None).is_empty());
        assert!(read_extension_prefs_from(Some(&serde_json::json!({"extensions": 4}))).is_empty());
    }

    /// An `extensions` entry that is not an object is not overwritten.
    #[test]
    fn a_malformed_extensions_entry_is_left_as_it_is() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{"extensions":[1,2]}"#).expect("write prefs.json");

        let refused = write_extension_pref_at(&path, "ext.sample.a", serde_json::json!(1));
        assert!(refused.is_err());
        let json = read_prefs_json(&path).expect("prefs.json parses");
        assert_eq!(json["extensions"], serde_json::json!([1, 2]));
    }
}
