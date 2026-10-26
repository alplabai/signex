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
    update_prefs_json(&prefs_path(), "first_run_tour_dismissed", |prefs| {
        prefs.insert(
            "first_run_tour_dismissed".to_string(),
            serde_json::Value::Bool(dismissed),
        );
    })
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

/// Writes the search boxes (Components filter + per-library Library
/// Browser queries) into `prefs.json`. The production store of the
/// debounced search-prefs loop; it runs on a background thread via `Task`,
/// never inside `update` (#859). The change is merged key by key into what
/// is on disk under the prefs IO lock, so entries this session never
/// touched survive.
pub struct PrefsSearchStore {
    path: PathBuf,
}

impl PrefsSearchStore {
    /// The store for the user's real `prefs.json`.
    pub fn user_prefs() -> Self {
        Self::at(prefs_path())
    }

    /// The store for an explicit prefs file (tests).
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
}

impl crate::persist::DebouncedStore<crate::persist::SearchPrefsChange> for PrefsSearchStore {
    fn write(&self, change: &crate::persist::SearchPrefsChange) -> Result<(), String> {
        try_update_prefs_json(&self.path, "search boxes", |prefs| {
            if let Some(filter) = &change.component_filter {
                prefs.insert(
                    "component_filter".to_string(),
                    serde_json::Value::String(filter.clone()),
                );
            }
            if change.library_searches.is_empty() {
                return;
            }
            let entry = prefs
                .entry("library_browser_searches".to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if !entry.is_object() {
                *entry = serde_json::Value::Object(serde_json::Map::new());
            }
            if let Some(searches) = entry.as_object_mut() {
                for (library, query) in &change.library_searches {
                    let key = library.display().to_string();
                    if query.is_empty() {
                        searches.remove(&key);
                    } else {
                        searches.insert(key, serde_json::Value::String(query.clone()));
                    }
                }
            }
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Wheel-zoom speed (percent per notch)
// ──────────────────────────────────────────────────────────────────────────

/// Read the wheel-zoom speed, in percent per notch. Defaults to 10 (a
/// x1.10 notch) when the key is missing or not a number; a number outside
/// 5..=50 is clamped into range.
pub fn read_zoom_step_percent_pref() -> u8 {
    read_zoom_step_percent_pref_at(&prefs_path())
}

pub fn read_zoom_step_percent_pref_at(path: &Path) -> u8 {
    read_prefs_json(path)
        .and_then(|json| json["zoom_step_percent"].as_f64())
        .filter(|v| v.is_finite())
        .map(|v| crate::canvas::Camera::clamp_zoom_step_percent(v.round() as i64))
        .unwrap_or(crate::canvas::Camera::ZOOM_STEP_PERCENT_DEFAULT)
}

pub fn write_zoom_step_percent_pref(percent: u8) {
    write_zoom_step_percent_pref_at(&prefs_path(), percent)
}

pub fn write_zoom_step_percent_pref_at(path: &Path, percent: u8) {
    update_prefs_json(path, "zoom_step_percent", |prefs| {
        prefs.insert("zoom_step_percent".to_string(), serde_json::json!(percent));
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Annotate undo scope
// ──────────────────────────────────────────────────────────────────────────

/// Read the "Annotate undo" preference. A missing or unknown value is the
/// default, one step for the whole project.
pub fn read_annotate_undo_pref() -> crate::annotate_undo::AnnotateUndo {
    read_annotate_undo_pref_at(&prefs_path())
}

pub fn read_annotate_undo_pref_at(path: &Path) -> crate::annotate_undo::AnnotateUndo {
    read_prefs_json(path)
        .and_then(|json| json["annotate_undo"].as_str().map(str::to_string))
        .map(|token| crate::annotate_undo::AnnotateUndo::from_token(&token))
        .unwrap_or_default()
}

pub fn write_annotate_undo_pref(scope: crate::annotate_undo::AnnotateUndo) {
    write_annotate_undo_pref_at(&prefs_path(), scope)
}

pub fn write_annotate_undo_pref_at(path: &Path, scope: crate::annotate_undo::AnnotateUndo) {
    update_prefs_json(path, "annotate_undo", |prefs| {
        prefs.insert(
            "annotate_undo".to_string(),
            serde_json::Value::String(scope.token().to_string()),
        );
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Keep wires connected when moving
// ──────────────────────────────────────────────────────────────────────────

/// Read "Keep wires connected when moving" (rubber-band moves, #963).
/// Defaults to on when the key is missing or not a boolean.
pub fn read_move_keeps_wires_pref() -> bool {
    read_move_keeps_wires_pref_at(&prefs_path())
}

pub fn read_move_keeps_wires_pref_at(path: &Path) -> bool {
    read_prefs_json(path)
        .and_then(|json| json["move_keeps_wires"].as_bool())
        .unwrap_or(true)
}

pub fn write_move_keeps_wires_pref(enabled: bool) {
    write_move_keeps_wires_pref_at(&prefs_path(), enabled)
}

pub fn write_move_keeps_wires_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, "move_keeps_wires", |prefs| {
        prefs.insert("move_keeps_wires".to_string(), serde_json::json!(enabled));
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Autosave recovery copies (#104)
// ──────────────────────────────────────────────────────────────────────────

/// Read the opt-in "Autosave recovery copies" toggle. Off when the key is
/// missing or not a boolean: nothing is written unless the user asked.
pub fn read_autosave_enabled_pref() -> bool {
    read_autosave_enabled_pref_at(&prefs_path())
}

pub fn read_autosave_enabled_pref_at(path: &Path) -> bool {
    read_prefs_json(path)
        .and_then(|json| json["autosave_enabled"].as_bool())
        .unwrap_or(false)
}

pub fn write_autosave_enabled_pref(enabled: bool) {
    write_autosave_enabled_pref_at(&prefs_path(), enabled)
}

pub fn write_autosave_enabled_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, "autosave_enabled", |prefs| {
        prefs.insert("autosave_enabled".to_string(), serde_json::json!(enabled));
    })
}
