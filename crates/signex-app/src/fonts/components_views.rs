//! The Components panel's saved state in `prefs.json` (mockup row 52):
//! the category shown last (Caner, 2026-11-27) and the column, sort and
//! group setup of each category, saved per user. Written off the UI thread
//! through `background_pref_write`; the write is the shared atomic
//! read-modify-write, so other keys are kept. Within the key, what this
//! build cannot read is kept too: a setup that does not parse, fields of a
//! setup it does not know and other subkeys (#3236), so a newer build's
//! state sharing the file survives.
//!
//! ```json
//! "components_panel_views": {
//!   "category": "capacitor",
//!   "setups": { "capacitor": { "order": ["row:part_number", "value"] } }
//! }
//! ```

use std::collections::BTreeMap;

use signex_library::components_view::ViewSetup;

use super::*;

/// The `prefs.json` key.
pub const COMPONENTS_VIEWS_KEY: &str = "components_panel_views";

/// What the key holds: the last category's key (`*` All parts, `@recent`
/// Recently placed, else a class) and the setups by category key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComponentsViews {
    pub category: Option<String>,
    pub setups: BTreeMap<String, ViewSetup>,
}

/// The saved state. A missing key or a value of the wrong shape reads as
/// none; one setup that does not parse (a newer build's) is skipped and the
/// others are kept.
pub fn read_components_views_from(json: Option<&serde_json::Value>) -> ComponentsViews {
    let Some(entry) = json.and_then(|j| j.get(COMPONENTS_VIEWS_KEY)) else {
        return ComponentsViews::default();
    };
    let category = entry
        .get("category")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty())
        .map(str::to_owned);
    let setups = entry
        .get("setups")
        .and_then(|s| s.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(category, value)| {
                    match serde_json::from_value::<ViewSetup>(value.clone()) {
                        Ok(setup) => Some((category.clone(), setup)),
                        Err(error) => {
                            tracing::warn!(
                                target: "signex::prefs",
                                category = %category,
                                error = %error,
                                "a saved Components panel setup did not parse; it is ignored"
                            );
                            None
                        }
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    ComponentsViews { category, setups }
}

/// Write the saved state, replacing the key.
pub fn write_components_views(views: &ComponentsViews) {
    write_components_views_at(&prefs_path(), views)
}

pub fn write_components_views_at(path: &Path, views: &ComponentsViews) {
    let setups = match serde_json::to_value(&views.setups) {
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
        let entry = merged_entry(
            prefs.get(COMPONENTS_VIEWS_KEY),
            views.category.as_deref(),
            setups,
        );
        prefs.insert(COMPONENTS_VIEWS_KEY.to_string(), entry);
    })
}

/// The fields [`ViewSetup`] reads; any other field of a saved setup is a
/// newer build's and is kept as written.
const SETUP_FIELDS: [&str; 6] = ["order", "hidden", "widths", "sort", "group", "pinned"];

/// The key's new value: `category` and `setups` (serialised from memory)
/// over what `old` holds. A saved setup that does not parse is kept unless
/// memory now has a setup for that category; unknown fields of a setup
/// and other subkeys are kept.
fn merged_entry(
    old: Option<&serde_json::Value>,
    category: Option<&str>,
    setups: serde_json::Value,
) -> serde_json::Value {
    use serde_json::Value;
    let mut entry = old.and_then(Value::as_object).cloned().unwrap_or_default();
    let old_setups = entry
        .get("setups")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut merged = match setups {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    for (key, old_setup) in old_setups {
        let readable = serde_json::from_value::<ViewSetup>(old_setup.clone()).is_ok();
        match merged.get_mut(&key).and_then(Value::as_object_mut) {
            Some(new_setup) => {
                if let Value::Object(old_fields) = old_setup {
                    for (field, value) in old_fields {
                        if !SETUP_FIELDS.contains(&field.as_str()) {
                            new_setup.entry(field).or_insert(value);
                        }
                    }
                }
            }
            // A readable setup memory no longer holds was reset: it goes.
            None if !readable => {
                merged.insert(key, old_setup);
            }
            None => {}
        }
    }
    match category {
        Some(category) => entry.insert("category".into(), Value::String(category.to_owned())),
        None => entry.remove("category"),
    };
    entry.insert("setups".into(), Value::Object(merged));
    Value::Object(entry)
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

    /// Setups are saved per user and the last category with them: both
    /// survive a save and a read unchanged, and other keys stay.
    #[test]
    fn saved_setups_and_the_last_category_round_trip_and_keep_other_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{"theme":"graphite_dark"}"#).expect("seed");
        let views = ComponentsViews {
            category: Some("capacitor".into()),
            setups: BTreeMap::from([
                ("capacitor".to_string(), setup()),
                ("*".to_string(), ViewSetup::default()),
            ]),
        };
        write_components_views_at(&path, &views);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert_eq!(json["theme"], "graphite_dark");
        assert_eq!(read_components_views_from(Some(&json)), views);
    }

    /// #3236: a write keeps what this build cannot read: a setup that
    /// does not parse, a setup's unknown field and another subkey. A setup
    /// it can read and no longer holds (reset) goes.
    #[test]
    fn a_write_keeps_the_setups_this_build_cannot_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        let mut known = serde_json::to_value(setup()).expect("value");
        known["density"] = serde_json::json!("compact");
        let reset = serde_json::to_value(ViewSetup::default()).expect("value");
        let seed = serde_json::json!({ COMPONENTS_VIEWS_KEY: {
            "category": "capacitor",
            "setups": {
                "capacitor": known,
                "inductor": { "order": 7, "layout": "cards" },
                "resistor": reset,
            },
            "favourites": ["capacitor"],
        }});
        std::fs::write(&path, seed.to_string()).expect("seed");
        let read = read_components_views_from(Some(&seed));
        assert_eq!(read.setups.len(), 2, "the inductor setup does not parse");
        let mut views = read.clone();
        views.setups.remove("resistor");
        views.category = Some("opamp".into());
        write_components_views_at(&path, &views);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        let entry = &json[COMPONENTS_VIEWS_KEY];
        assert_eq!(entry["category"], "opamp");
        assert_eq!(entry["favourites"], serde_json::json!(["capacitor"]));
        assert_eq!(
            entry["setups"]["inductor"],
            serde_json::json!({ "order": 7, "layout": "cards" })
        );
        assert_eq!(entry["setups"]["capacitor"]["density"], "compact");
        assert!(entry["setups"].get("resistor").is_none());
        assert_eq!(read_components_views_from(Some(&json)).setups, views.setups);
    }

    #[test]
    fn a_missing_key_or_a_bad_entry_reads_as_nothing_saved() {
        assert_eq!(read_components_views_from(None), ComponentsViews::default());
        let json = serde_json::json!({ COMPONENTS_VIEWS_KEY: 3 });
        assert_eq!(
            read_components_views_from(Some(&json)),
            ComponentsViews::default()
        );
        let good = serde_json::to_value(setup()).expect("value");
        let json = serde_json::json!({ COMPONENTS_VIEWS_KEY: {
            "category": "@recent",
            "setups": { "capacitor": good, "resistor": { "order": 7 } },
        }});
        let read = read_components_views_from(Some(&json));
        assert_eq!(read.category.as_deref(), Some("@recent"));
        assert_eq!(read.setups.len(), 1);
        assert_eq!(read.setups["capacitor"], setup());
    }
}
