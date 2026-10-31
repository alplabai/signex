//! Misc UI preference IO (first-run tour, component filter, browser searches). Split from `fonts.rs`.

use super::*;
use crate::chrome::controls::WindowControlsPlacement;

// ──────────────────────────────────────────────────────────────────────────
// First-run tour (UX_IMPROVEMENTS_OVER_ALTIUM §4.3)
// ──────────────────────────────────────────────────────────────────────────

/// Has the user dismissed the first-run tour card? Default `false` so a
/// fresh install shows the card on first launch; once dismissed (via the
/// X button, Esc, or any canvas interaction) the flag flips to `true`
/// and stays that way for the lifetime of the prefs file.
pub fn read_first_run_tour_dismissed() -> bool {
    read_first_run_tour_dismissed_from(load_prefs_json().as_ref())
}

pub fn read_first_run_tour_dismissed_from(json: Option<&serde_json::Value>) -> bool {
    let Some(json) = json else {
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
// Stock primitive copy target (#813)
// ──────────────────────────────────────────────────────────────────────────

/// The `.snxlib` the user last chose to receive copies of stock symbols and
/// footprints, from `prefs.json`'s `stock_copy_library`. `None` when never
/// chosen or not a non-empty string.
pub fn read_stock_copy_target_pref_from(json: Option<&serde_json::Value>) -> Option<PathBuf> {
    let text = json?["stock_copy_library"].as_str()?;
    (!text.is_empty()).then(|| PathBuf::from(text))
}

/// Persist the stock-copy target without clobbering other keys.
pub fn write_stock_copy_target_pref(target: &Path) {
    write_stock_copy_target_pref_at(&prefs_path(), target)
}

pub fn write_stock_copy_target_pref_at(path: &Path, target: &Path) {
    update_prefs_json(path, "stock_copy_library", |prefs| {
        prefs.insert(
            "stock_copy_library".to_string(),
            serde_json::Value::String(target.display().to_string()),
        );
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Persistent search queries (UX_IMPROVEMENTS_OVER_ALTIUM §1.1)
// ──────────────────────────────────────────────────────────────────────────

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

/// Writes the search boxes (per-library Library Browser queries) into `prefs.json`. The production store of the
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
    read_zoom_step_percent_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_zoom_step_percent_pref_from(json: Option<&serde_json::Value>) -> u8 {
    json.and_then(|json| json["zoom_step_percent"].as_f64())
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
    read_annotate_undo_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_annotate_undo_pref_from(
    json: Option<&serde_json::Value>,
) -> crate::annotate_undo::AnnotateUndo {
    json.and_then(|json| json["annotate_undo"].as_str().map(str::to_string))
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
    read_move_keeps_wires_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_move_keeps_wires_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["move_keeps_wires"].as_bool())
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
// Net insight on hover
// ──────────────────────────────────────────────────────────────────────────

/// Read "Show net info on hover" (#1270). Defaults to on when the key is
/// missing or not a boolean.
pub fn read_net_hover_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["net_hover_enabled"].as_bool())
        .unwrap_or(true)
}

pub fn write_net_hover_pref(enabled: bool) {
    write_net_hover_pref_at(&prefs_path(), enabled)
}

pub fn write_net_hover_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, "net_hover_enabled", |prefs| {
        prefs.insert("net_hover_enabled".to_string(), serde_json::json!(enabled));
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Select by drag direction
// ──────────────────────────────────────────────────────────────────────────

/// Read "Select by drag direction" (#1257). Defaults to on when the key is
/// missing or not a boolean.
pub fn read_drag_direction_select_pref_at(path: &Path) -> bool {
    read_drag_direction_select_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_drag_direction_select_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["drag_direction_select"].as_bool())
        .unwrap_or(true)
}

pub fn write_drag_direction_select_pref(enabled: bool) {
    write_drag_direction_select_pref_at(&prefs_path(), enabled)
}

pub fn write_drag_direction_select_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, "drag_direction_select", |prefs| {
        prefs.insert(
            "drag_direction_select".to_string(),
            serde_json::json!(enabled),
        );
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Increment step and designators on paste
// ──────────────────────────────────────────────────────────────────────────

/// Read the "Increment step" (#1253). A missing, non-numeric or zero value is
/// the default; one outside the allowed range is clamped.
pub fn read_increment_step_pref_from(json: Option<&serde_json::Value>) -> i32 {
    json.and_then(|json| json["increment_step"].as_i64())
        .map(signex_types::text_increment::clamp_step)
        .unwrap_or(signex_types::text_increment::STEP_DEFAULT)
}

pub fn write_increment_step_pref(step: i32) {
    write_increment_step_pref_at(&prefs_path(), step)
}

pub fn write_increment_step_pref_at(path: &Path, step: i32) {
    update_prefs_json(path, "increment_step", |prefs| {
        prefs.insert("increment_step".to_string(), serde_json::json!(step));
    })
}

/// Marker stored beside `paste_designators` once the value is one the user
/// chose under the "Keep unless taken" default (#1768). A stored `keep`
/// without it was written by an older build, when Keep was the default.
const PASTE_DESIGNATORS_SCHEMA: u64 = 2;
const PASTE_DESIGNATORS_SCHEMA_KEY: &str = "paste_designators_schema";

/// Whether the file holds a legacy `keep` that has not been migrated yet.
pub fn paste_designators_needs_migration(json: Option<&serde_json::Value>) -> bool {
    let Some(json) = json else { return false };
    let keep = json["paste_designators"].as_str().is_some_and(|t| {
        t.eq_ignore_ascii_case(crate::paste_prefs::PasteDesignators::Keep.token())
    });
    let current = json[PASTE_DESIGNATORS_SCHEMA_KEY]
        .as_u64()
        .is_some_and(|v| v >= PASTE_DESIGNATORS_SCHEMA);
    keep && !current
}

/// Read "Designators on paste" (#1263). A missing or unknown value is Keep
/// unless taken, and so is a legacy `keep` (see [`PASTE_DESIGNATORS_SCHEMA`]).
pub fn read_paste_designators_pref_from(
    json: Option<&serde_json::Value>,
) -> crate::paste_prefs::PasteDesignators {
    if paste_designators_needs_migration(json) {
        return crate::paste_prefs::PasteDesignators::KeepIfFree;
    }
    json.and_then(|json| json["paste_designators"].as_str())
        .map(crate::paste_prefs::PasteDesignators::from_token)
        .unwrap_or_default()
}

/// Rewrite a legacy `keep` as `keep_if_free` with the schema marker, once.
/// Returns whether it changed the file.
pub fn migrate_paste_designators_pref_at(path: &Path) -> bool {
    if !paste_designators_needs_migration(read_prefs_json(path).as_ref()) {
        return false;
    }
    write_paste_designators_pref_at(path, crate::paste_prefs::PasteDesignators::KeepIfFree);
    true
}

/// [`migrate_paste_designators_pref_at`] on the user's prefs file.
pub fn migrate_paste_designators_pref() -> bool {
    migrate_paste_designators_pref_at(&prefs_path())
}

pub fn write_paste_designators_pref(mode: crate::paste_prefs::PasteDesignators) {
    write_paste_designators_pref_at(&prefs_path(), mode)
}

pub fn write_paste_designators_pref_at(path: &Path, mode: crate::paste_prefs::PasteDesignators) {
    update_prefs_json(path, "paste_designators", |prefs| {
        prefs.insert(
            "paste_designators".to_string(),
            serde_json::Value::String(mode.token().to_string()),
        );
        prefs.insert(
            PASTE_DESIGNATORS_SCHEMA_KEY.to_string(),
            serde_json::json!(PASTE_DESIGNATORS_SCHEMA),
        );
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Window controls placement
// ──────────────────────────────────────────────────────────────────────────

/// Read where the main window's min/max/close buttons go. Anything missing
/// or unrecognised is `Auto`.
pub fn read_window_controls_placement_pref() -> WindowControlsPlacement {
    read_window_controls_placement_pref_at(&prefs_path())
}

pub fn read_window_controls_placement_pref_at(path: &Path) -> WindowControlsPlacement {
    read_window_controls_placement_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_window_controls_placement_pref_from(
    json: Option<&serde_json::Value>,
) -> WindowControlsPlacement {
    json.and_then(|json| json["window_controls_placement"].as_str())
        .map(WindowControlsPlacement::from_token)
        .unwrap_or_default()
}

pub fn write_window_controls_placement_pref(placement: WindowControlsPlacement) {
    write_window_controls_placement_pref_at(&prefs_path(), placement)
}

pub fn write_window_controls_placement_pref_at(path: &Path, placement: WindowControlsPlacement) {
    update_prefs_json(path, "window_controls_placement", |prefs| {
        prefs.insert(
            "window_controls_placement".to_string(),
            serde_json::json!(placement.token()),
        );
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
    read_autosave_enabled_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_autosave_enabled_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["autosave_enabled"].as_bool())
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

// ──────────────────────────────────────────────────────────────────────────
// Interface scale (#1228)
// ──────────────────────────────────────────────────────────────────────────

/// Read the interface scale. Missing is 1.0; an out-of-range value is
/// clamped and a malformed one falls back to 1.0, both reported to the
/// Messages panel.
pub fn read_ui_scale_pref() -> f32 {
    read_ui_scale_pref_at(&prefs_path())
}

pub fn read_ui_scale_pref_at(path: &Path) -> f32 {
    read_ui_scale_pref_from(read_prefs_json(path).as_ref(), path)
}

/// [`read_ui_scale_pref_at`] over an already-parsed `json`; `path` only
/// names the file in the warning.
pub fn read_ui_scale_pref_from(json: Option<&serde_json::Value>, path: &Path) -> f32 {
    let read = crate::ui_scale::parse_ui_scale(json.and_then(|j| j.get("ui_scale")));
    if let Some(problem) = &read.problem {
        crate::diagnostics::log_warning(format!("{}: {problem}", path.display()));
    }
    read.scale
}

pub fn write_ui_scale_pref(scale: f32) {
    write_ui_scale_pref_at(&prefs_path(), scale)
}

pub fn write_ui_scale_pref_at(path: &Path, scale: f32) {
    update_prefs_json(path, "ui_scale", |prefs| {
        prefs.insert(
            "ui_scale".to_string(),
            serde_json::json!(crate::ui_scale::scale_for_file(scale)),
        );
    })
}
