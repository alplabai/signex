//! Filter-preset preference IO. Split from `fonts.rs`.

use super::*;

/// Read the user-defined custom selection-filter presets. Returns an
/// empty `Vec` if the file is missing, malformed, or the key absent.
/// Capped to `CUSTOM_FILTER_PRESET_LIMIT` entries on read so a hand-
/// edited file with too many slots still loads cleanly.
pub fn read_custom_filter_presets() -> Vec<crate::active_bar::CustomFilterPreset> {
    use crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT;
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    let Some(array) = json.get("custom_filter_presets").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    array
        .iter()
        .take(CUSTOM_FILTER_PRESET_LIMIT)
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect()
}

/// Persist the list of custom selection-filter presets without
/// clobbering other preference keys.
pub fn write_custom_filter_presets(presets: &[crate::active_bar::CustomFilterPreset]) {
    update_prefs_json(&prefs_path(), "custom_filter_presets", |prefs| {
        if let Ok(array) = serde_json::to_value(presets) {
            prefs.insert("custom_filter_presets".to_string(), array);
        }
    })
}

/// Read the user-defined footprint-editor filter presets. Returns an
/// empty `Vec` if the file is missing, malformed, or the key absent.
/// Capped to `CUSTOM_FILTER_PRESET_LIMIT` entries on read so a hand-
/// edited file with too many slots still loads cleanly. Parallel to
/// `read_custom_filter_presets` (schematic), but keyed on
/// `FootprintFilterPreset` (Task 6).
pub fn read_footprint_filter_presets() -> Vec<crate::active_bar::FootprintFilterPreset> {
    use crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT;
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    let Some(array) = json
        .get("footprint_filter_presets")
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    array
        .iter()
        .take(CUSTOM_FILTER_PRESET_LIMIT)
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect()
}

/// Persist the list of footprint-editor filter presets without
/// clobbering other preference keys.
pub fn write_footprint_filter_presets(presets: &[crate::active_bar::FootprintFilterPreset]) {
    update_prefs_json(&prefs_path(), "footprint_filter_presets", |prefs| {
        if let Ok(array) = serde_json::to_value(presets) {
            prefs.insert("footprint_filter_presets".to_string(), array);
        }
    })
}
