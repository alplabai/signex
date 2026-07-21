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
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    if let Ok(array) = serde_json::to_value(presets) {
        json["custom_filter_presets"] = array;
    }
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "fonts_pref");
    }
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
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    if let Ok(array) = serde_json::to_value(presets) {
        json["footprint_filter_presets"] = array;
    }
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "fonts_pref");
    }
}
