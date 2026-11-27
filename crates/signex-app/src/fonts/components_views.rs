//! The Components panel's saved column, sort and group setups, one per
//! category, in `prefs.json` (components-cx decision 1). Written off the
//! UI thread through `background_pref_write`; the write is the shared
//! atomic read-modify-write, so other keys are kept.

use std::collections::BTreeMap;

use signex_library::components_view::ViewSetup;

use super::*;

/// The `prefs.json` key: an object of category key to setup.
pub const COMPONENTS_VIEWS_KEY: &str = "components_panel_views";

/// The saved setups. A missing key or a value that is not an object reads
/// as none; one entry that does not parse (a newer build's) is skipped and
/// the others are kept.
pub fn read_components_view_setups_from(
    json: Option<&serde_json::Value>,
) -> BTreeMap<String, ViewSetup> {
    let Some(map) = json.and_then(|j| j.get(COMPONENTS_VIEWS_KEY)?.as_object().cloned()) else {
        return BTreeMap::new();
    };
    map.into_iter()
        .filter_map(
            |(category, value)| match serde_json::from_value::<ViewSetup>(value) {
                Ok(setup) => Some((category, setup)),
                Err(error) => {
                    tracing::warn!(
                        target: "signex::prefs",
                        category = %category,
                        error = %error,
                        "a saved Components panel setup did not parse; it is ignored"
                    );
                    None
                }
            },
        )
        .collect()
}

/// Write every saved setup, replacing the key.
pub fn write_components_view_setups(setups: &BTreeMap<String, ViewSetup>) {
    write_components_view_setups_at(&prefs_path(), setups)
}

pub fn write_components_view_setups_at(path: &Path, setups: &BTreeMap<String, ViewSetup>) {
    let value = match serde_json::to_value(setups) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(
                target: "signex::prefs",
                error = %error,
                "the Components panel setups could not be serialised; not saved"
            );
            return;
        }
    };
    update_prefs_json(path, COMPONENTS_VIEWS_KEY, |prefs| {
        prefs.insert(COMPONENTS_VIEWS_KEY.to_string(), value);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::templates::{ColumnKey, SortKey};

    fn setup() -> ViewSetup {
        let mut s = ViewSetup {
            order: vec![
                ColumnKey::PartNumber,
                ColumnKey::param("value"),
                ColumnKey::param("voltage"),
                ColumnKey::Stock,
            ],
            ..ViewSetup::default()
        };
        s.hidden.insert(ColumnKey::Stock);
        s.widths.insert(ColumnKey::param("value"), 96.0);
        s.sort = vec![SortKey::descending(ColumnKey::param("voltage"))];
        s.group = vec![SortKey::ascending(ColumnKey::param("package"))];
        s.pinned = vec!["value".into()];
        s
    }

    /// Decision 1: a setup survives a save and a read unchanged, and other
    /// keys of `prefs.json` stay.
    #[test]
    fn saved_setups_round_trip_through_prefs_and_keep_other_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{"theme":"graphite_dark"}"#).expect("seed");
        let setups = BTreeMap::from([
            ("capacitor".to_string(), setup()),
            ("*".to_string(), ViewSetup::default()),
        ]);
        write_components_view_setups_at(&path, &setups);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert_eq!(json["theme"], "graphite_dark");
        assert_eq!(read_components_view_setups_from(Some(&json)), setups);
    }

    #[test]
    fn a_missing_key_or_a_bad_entry_reads_as_no_setup() {
        assert!(read_components_view_setups_from(None).is_empty());
        let json = serde_json::json!({ COMPONENTS_VIEWS_KEY: 3 });
        assert!(read_components_view_setups_from(Some(&json)).is_empty());
        let good = serde_json::to_value(setup()).expect("value");
        let json = serde_json::json!({ COMPONENTS_VIEWS_KEY: {
            "capacitor": good,
            "resistor": { "order": 7 },
        }});
        let read = read_components_view_setups_from(Some(&json));
        assert_eq!(read.len(), 1);
        assert_eq!(read["capacitor"], setup());
    }
}
