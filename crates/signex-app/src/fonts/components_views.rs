//! The Components panel's saved state in `prefs.json` (mockup row 52):
//! the category shown last (Caner, 2026-11-27) and the column, sort and
//! group setup of each category, saved per user. Written off the UI thread
//! through `background_pref_write`; the write is the shared atomic
//! read-modify-write, so other keys are kept.
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
    let mut entry = serde_json::Map::new();
    if let Some(category) = &views.category {
        entry.insert(
            "category".into(),
            serde_json::Value::String(category.clone()),
        );
    }
    entry.insert("setups".into(), setups);
    update_prefs_json(path, COMPONENTS_VIEWS_KEY, |prefs| {
        prefs.insert(
            COMPONENTS_VIEWS_KEY.to_string(),
            serde_json::Value::Object(entry),
        );
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
