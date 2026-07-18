//! Misc UI preference IO (first-run tour, component filter, browser searches). Split from `fonts.rs`.

use super::*;

// ──────────────────────────────────────────────────────────────────────────
// First-run tour (UX_IMPROVEMENTS_OVER_ALTIUM §4.3)
// ──────────────────────────────────────────────────────────────────────────

/// Has the user dismissed the first-run tour card? Default `false` so a
/// fresh install shows the card on first launch; once dismissed (via the
/// X button, Esc, or any canvas interaction) the flag flips to `true`
/// and stays that way for the lifetime of the prefs file.
pub fn read_first_run_tour_dismissed() -> bool {
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    json["first_run_tour_dismissed"].as_bool().unwrap_or(false)
}

/// Persist the dismissal flag without clobbering other keys.
pub fn write_first_run_tour_dismissed(dismissed: bool) {
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    json["first_run_tour_dismissed"] = serde_json::Value::Bool(dismissed);
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "fonts_pref");
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Persistent search queries (UX_IMPROVEMENTS_OVER_ALTIUM §1.1)
// ──────────────────────────────────────────────────────────────────────────

/// Read the last-typed Components-panel filter, if any. Empty string
/// when missing or malformed — that's the same as a fresh session for
/// the panel.
pub fn read_component_filter() -> String {
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return String::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return String::new();
    };
    json["component_filter"].as_str().unwrap_or("").to_string()
}

/// Persist the Components-panel filter without clobbering other keys.
pub fn write_component_filter(query: &str) {
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    json["component_filter"] = serde_json::Value::String(query.to_string());
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "fonts_pref");
    }
}

/// Read the persisted per-`.snxlib` Library Browser search queries.
/// Keyed by the absolute path's display string; entries for libraries
/// that no longer exist on disk are harmless — they're loaded but only
/// touched again when the user reopens that library.
pub fn read_library_browser_searches() -> std::collections::HashMap<PathBuf, String> {
    let mut out = std::collections::HashMap::new();
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return out;
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return out;
    };
    let Some(obj) = json
        .get("library_browser_searches")
        .and_then(|v| v.as_object())
    else {
        return out;
    };
    for (k, v) in obj {
        let Some(s) = v.as_str() else { continue };
        if s.is_empty() {
            continue;
        }
        out.insert(PathBuf::from(k), s.to_string());
    }
    out
}

/// Persist a single library's search query. Reading the existing map
/// from disk first means concurrent updates to other libraries don't
/// stomp each other (same-process only — cross-process serialisation
/// is out of scope for prefs).
pub fn write_library_browser_search(library_path: &std::path::Path, query: &str) {
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    let key = library_path.display().to_string();
    let map = json
        .as_object_mut()
        .expect("prefs root is always an object");
    let entry = map
        .entry("library_browser_searches".to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Some(obj) = entry.as_object_mut() {
        if query.is_empty() {
            obj.remove(&key);
        } else {
            obj.insert(key, serde_json::Value::String(query.to_string()));
        }
    }
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "fonts_pref");
    }
}

