//! Filter-preset preference IO. Split from `fonts.rs`.

use super::*;
use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, CustomFilterPreset, PresetKind};

/// The `prefs.json` key of the schematic's selection-filter presets.
pub const SCHEMATIC_FILTER_PRESETS: &str = "custom_filter_presets";
/// The `prefs.json` key of the board's selection-filter presets (#2383).
pub const BOARD_FILTER_PRESETS: &str = "pcb_filter_presets";

/// Read the user-defined custom selection-filter presets. Returns an
/// empty `Vec` if the file is missing, malformed, or the key absent.
pub fn read_custom_filter_presets() -> Vec<CustomFilterPreset> {
    read_custom_filter_presets_from(load_prefs_json().as_ref())
}

pub fn read_custom_filter_presets_from(
    json: Option<&serde_json::Value>,
) -> Vec<CustomFilterPreset> {
    read_filter_presets_from(json, SCHEMATIC_FILTER_PRESETS)
}

/// Read the presets saved under `key`. Returns an empty `Vec` if the file
/// is missing, malformed, or the key absent; a preset that does not read
/// is skipped. Capped to `CUSTOM_FILTER_PRESET_LIMIT` entries on read so a
/// hand-edited file with too many slots still loads cleanly.
pub fn read_filter_presets_from<K: PresetKind>(
    json: Option<&serde_json::Value>,
    key: &str,
) -> Vec<CustomFilterPreset<K>> {
    let Some(array) = json
        .and_then(|json| json.get(key))
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

/// Persist the list of custom selection-filter presets without
/// clobbering other preference keys.
pub fn write_custom_filter_presets(presets: &[CustomFilterPreset]) {
    write_filter_presets(SCHEMATIC_FILTER_PRESETS, presets)
}

/// Persist `presets` under `key` without clobbering other preference keys.
pub fn write_filter_presets<K: PresetKind>(key: &'static str, presets: &[CustomFilterPreset<K>]) {
    update_prefs_json(&prefs_path(), key, |prefs| {
        if let Ok(array) = serde_json::to_value(presets) {
            prefs.insert(key.to_string(), array);
        }
    })
}

/// Read the user-defined footprint-editor filter presets. Returns an
/// empty `Vec` if the file is missing, malformed, or the key absent.
/// Capped to `CUSTOM_FILTER_PRESET_LIMIT` entries on read so a hand-
/// edited file with too many slots still loads cleanly. Parallel to
/// `read_custom_filter_presets` (schematic), but keyed on
/// `FootprintFilterPreset` (Task 6). Startup-only: handlers take the list
/// from `interaction_state`, never from the disk.
pub fn read_footprint_filter_presets_from(
    json: Option<&serde_json::Value>,
) -> Vec<crate::active_bar::FootprintFilterPreset> {
    let Some(json) = json else {
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
