//! Font management for Signex.
//!
//! Responsibilities:
//! - Enumerate system font families using fontdb (done once, cached).
//! - Provide the canonical canvas font constant (Iosevka).
//! - Read / write the UI font preference from a simple JSON config file.
//!
//! Config file: OS-canonical config dir (`%APPDATA%\signex\prefs.json`
//! on Windows, `~/Library/Application Support/signex/prefs.json` on
//! macOS, `$XDG_CONFIG_HOME/signex/prefs.json` on Linux).
//! Format: `{"ui_font": "Roboto"}`

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::render_config::{GridStyle, LabelStyle, MultisheetStyle, PowerPortStyle};
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

/// Default UI font family name. Used when no preference file is found.
pub const DEFAULT_UI_FONT: &str = "Roboto";

/// MD-32: persist `bytes` to `path` atomically (tmp + rename via
/// `signex_types::atomic_io`) and `tracing::debug!` on failure
/// instead of the `let _ = std::fs::write(...)` pattern that swallows
/// disk-full / permission errors silently. Used by every preferences
/// write in this module so a single source of failures shows up in
/// `RUST_LOG=signex_app::fonts=debug` instead of nowhere.
fn write_pref_atomic(path: &Path, bytes: &[u8], context: &str) {
    if let Err(e) = signex_types::atomic_io::atomic_write(path, bytes) {
        tracing::debug!(
            target = "signex::prefs",
            path = %path.display(),
            context = context,
            error = %e,
            "preference write failed (best-effort, will retry on next change)"
        );
    }
}

/// Default canvas (schematic / PCB) font family name.
/// Iosevka is bundled in `assets/fonts/` and loaded at startup.
pub const DEFAULT_CANVAS_FONT: &str = "Iosevka";

/// Default seed component class list — used when `prefs.json` carries
/// no `component_classes` array (fresh install / user hasn't customised
/// the list yet). The first element of each tuple is the canonical key
/// stored on `ComponentRow.class`; the second is the user-facing label
/// shown in pickers and editors. Users can add / rename / delete via
/// Preferences ▸ Component Classes; the customised list persists as
/// the `component_classes` array in `prefs.json`.
pub const DEFAULT_COMPONENT_CLASSES: &[(&str, &str)] = &[
    ("resistor", "Resistor"),
    ("capacitor", "Capacitor"),
    ("inductor", "Inductor"),
    ("diode", "Diode"),
    ("led", "LED"),
    ("transistor_bjt", "Transistor — BJT"),
    ("transistor_mosfet", "Transistor — MOSFET"),
    ("transistor_jfet", "Transistor — JFET"),
    ("opamp", "Op-Amp"),
    ("comparator", "Comparator"),
    ("regulator_linear", "Regulator — Linear"),
    ("regulator_switching", "Regulator — Switching"),
    ("mcu", "MCU"),
    ("logic", "Logic"),
    ("memory", "Memory"),
    ("connector", "Connector"),
    ("switch", "Switch"),
    ("relay", "Relay"),
    ("crystal", "Crystal / Oscillator"),
    ("transformer", "Transformer"),
    ("fuse", "Fuse"),
    ("antenna", "Antenna"),
    ("display", "Display"),
    ("sensor", "Sensor"),
    ("motor", "Motor"),
    ("battery", "Battery"),
    ("generic", "Generic"),
];

/// One entry in the user's component-class list. Persisted as a JSON
/// object `{ "key": "...", "label": "..." }` inside the
/// `component_classes` array in `prefs.json`. `key` is the canonical
/// machine identifier stored on `ComponentRow.class`; `label` is the
/// human-readable name surfaced in pickers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComponentClassEntry {
    pub key: String,
    pub label: String,
}

/// Materialise the seed list as owned `ComponentClassEntry` values.
pub fn default_component_classes() -> Vec<ComponentClassEntry> {
    DEFAULT_COMPONENT_CLASSES
        .iter()
        .map(|(k, l)| ComponentClassEntry {
            key: (*k).to_string(),
            label: (*l).to_string(),
        })
        .collect()
}

/// Build an [`iced::Font`] that targets the given family name. iced's
/// `Font::with_name` requires `&'static str` because the renderer
/// caches by family name, but the Preferences panel hands us font
/// names as runtime `String`s. The intern map below leaks one
/// `&'static str` per unique family name ever resolved during a
/// session — bounded by the small set of fonts a user actually picks,
/// so the cumulative leak is negligible. Used for surfaces that need
/// the canvas font (Iosevka by default) — e.g. the symbol hover
/// tooltip.
pub fn iced_font_for_family(name: &str) -> iced::Font {
    static INTERN: OnceLock<Mutex<std::collections::HashMap<String, &'static str>>> =
        OnceLock::new();
    let map_lock = INTERN.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    // `unwrap_or_else(|e| e.into_inner())` recovers from a poisoned
    // mutex — the inner value is still valid (a HashMap of leaked
    // names; nothing in flight here can corrupt it). Using
    // `.unwrap()` would panic the UI thread on every subsequent
    // tooltip render after any unrelated panic that happened to
    // hold this lock.
    let mut map = map_lock.lock().unwrap_or_else(|e| e.into_inner());
    let static_name: &'static str = match map.get(name) {
        Some(s) => *s,
        None => {
            let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
            map.insert(name.to_string(), leaked);
            leaked
        }
    };
    iced::Font::with_name(static_name)
}

// ──────────────────────────────────────────────────────────────────────────
// System font enumeration
// ──────────────────────────────────────────────────────────────────────────

/// Return the list of distinct font family names available on this system.
///
/// Expensive on first call (scans system font directories via fontdb),
/// then cached for the lifetime of the process.
pub fn system_font_families() -> &'static Vec<String> {
    static CACHE: OnceLock<Vec<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        // Collect all unique family names, sorted alphabetically.
        let mut families: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for face in db.faces() {
            if let Some((name, _)) = face.families.first() {
                families.insert(name.clone());
            }
        }

        // Always ensure Iosevka appears (it is bundled) so the canvas font
        // option is always present even if not installed system-wide.
        families.insert(DEFAULT_CANVAS_FONT.to_string());

        families.into_iter().collect()
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Preferences file
// ──────────────────────────────────────────────────────────────────────────

/// Canonical OS-native preferences-file location:
/// - Windows: `%APPDATA%\signex\prefs.json`
/// - macOS:   `~/Library/Application Support/signex/prefs.json`
/// - Linux:   `$XDG_CONFIG_HOME/signex/prefs.json` (or `~/.config/...`)
///
/// Computed once per process (via `OnceLock`) so the legacy-prefs
/// migration runs at most once.
fn prefs_path() -> PathBuf {
    use std::sync::OnceLock;
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let canonical = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("signex")
            .join("prefs.json");
        let legacy = legacy_posix_prefs_path();
        migrate_legacy_prefs(&canonical, &legacy);
        canonical
    })
    .clone()
}

/// One-shot startup migrations applied to `prefs.json` before any
/// reader sees the file. Idempotent — runs at most once per process
/// because [`prefs_path`] caches via `OnceLock`.
///
/// **F1** (Windows prefs path bug): pre-v0.12 the path was hardcoded
/// to `$XDG_CONFIG_HOME/signex/prefs.json` or
/// `$HOME/.config/signex/prefs.json`. On Windows that landed in a
/// `.config` subfolder of the user dir rather than the canonical
/// `%APPDATA%\signex\`. If the legacy path has a file but the
/// canonical path doesn't, copy it forward.
///
/// **F3** (stale label-style discriminants): pre-v0.10 prefs files
/// carried label-style tokens that aren't in the current canonical
/// set. The reader silently falls through to `LabelStyle::Standard`
/// for unknown discriminants, but the literal stale string lingers
/// in user-space `prefs.json` until the user changes label style +
/// saves. Rewrite once on startup so unknown tokens normalise to
/// the canonical default.
///
/// `legacy` is the legacy (pre-v0.12) POSIX-shaped prefs path to
/// pull from when canonical is missing. Production passes
/// [`legacy_posix_prefs_path()`]; tests inject a tempdir-shaped path.
pub fn migrate_legacy_prefs(canonical: &Path, legacy: &Path) {
    // F1: copy legacy path → canonical, but only if canonical is empty.
    if !canonical.exists() {
        if legacy != canonical && legacy.exists() {
            if let Some(parent) = canonical.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(legacy, canonical);
        }
    }

    // F3: rewrite any non-canonical label_style discriminant → "standard".
    // Canonical writers emit lowercase "standard" / "altium"; we accept any
    // case match and rewrite anything else to the default.
    const CANONICAL_LABEL_STYLES: &[&str] = &["standard", "altium"];
    let Ok(bytes) = std::fs::read(canonical) else {
        return;
    };
    let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return;
    };
    let stale_label = json
        .get("label_style")
        .and_then(|v| v.as_str())
        .map(|s| {
            !CANONICAL_LABEL_STYLES
                .iter()
                .any(|c| s.eq_ignore_ascii_case(c))
        })
        .unwrap_or(false);
    if stale_label {
        json["label_style"] = serde_json::Value::String("standard".to_string());
        if let Ok(serialized) = serde_json::to_string_pretty(&json) {
            write_pref_atomic(
                canonical,
                serialized.as_bytes(),
                "migrate_legacy_label_style",
            );
        }
    }
}

/// Pre-v0.12 prefs path: POSIX-style under `$XDG_CONFIG_HOME` or
/// `$HOME/.config`. Only used by [`migrate_legacy_prefs`] to find
/// existing user files for one-shot migration.
fn legacy_posix_prefs_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("signex").join("prefs.json")
}

/// Read only the `ui_font` key from the preferences file.
/// Returns `DEFAULT_UI_FONT` if the file is absent, malformed, or the key missing.
pub fn read_ui_font_pref() -> String {
    read_ui_font_pref_at(&prefs_path())
}

pub fn read_ui_font_pref_at(path: &Path) -> String {
    read_prefs_json(path)
        .and_then(|j| j["ui_font"].as_str().map(str::to_string))
        .unwrap_or_else(|| DEFAULT_UI_FONT.to_string())
}

/// Persist the given `ui_font` choice to the preferences file.
/// Creates parent directories if they do not exist.
/// Silently ignores I/O errors (non-critical preference).
pub fn write_ui_font_pref(font_name: &str) {
    write_ui_font_pref_at(&prefs_path(), font_name)
}

pub fn write_ui_font_pref_at(path: &Path, font_name: &str) {
    update_prefs_json(path, |json| {
        json["ui_font"] = serde_json::Value::String(font_name.to_string());
    })
}

/// Read the user's component-class list from the prefs file. Falls
/// back to [`default_component_classes`] only when the file is
/// absent / malformed, or when the `component_classes` key is
/// missing entirely (a fresh install). A user who has the array
/// present and empty is honored verbatim — the New Component
/// surface handles the "no classes defined" case at the point of
/// use, so saving an empty list and reading it back round-trips
/// faithfully.
pub fn read_component_classes_pref() -> Vec<ComponentClassEntry> {
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return default_component_classes();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return default_component_classes();
    };
    let Some(arr) = json["component_classes"].as_array() else {
        return default_component_classes();
    };
    arr.iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect()
}

/// Persist `classes` to the `component_classes` array in prefs.json
/// without clobbering other preference keys. Silent on I/O failure
/// — preferences are best-effort.
pub fn write_component_classes_pref(classes: &[ComponentClassEntry]) {
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    if let Ok(value) = serde_json::to_value(classes) {
        json["component_classes"] = value;
    }
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "component_classes");
    }
}

/// Read `power_port_style` from preferences file.
/// Defaults to `Altium` when missing or invalid.
pub fn read_power_port_style_pref() -> PowerPortStyle {
    read_power_port_style_pref_at(&prefs_path())
}

pub fn read_power_port_style_pref_at(path: &Path) -> PowerPortStyle {
    let raw = read_prefs_json(path)
        .and_then(|j| j["power_port_style"].as_str().map(str::to_string))
        .unwrap_or_default();
    if raw.eq_ignore_ascii_case("standard") {
        PowerPortStyle::Standard
    } else {
        PowerPortStyle::Altium
    }
}

/// Persist power port style without clobbering other preference keys.
pub fn write_power_port_style_pref(style: PowerPortStyle) {
    write_power_port_style_pref_at(&prefs_path(), style)
}

pub fn write_power_port_style_pref_at(path: &Path, style: PowerPortStyle) {
    let token = match style {
        PowerPortStyle::Standard => "standard",
        PowerPortStyle::Altium => "altium",
    };
    update_prefs_json(path, |json| {
        json["power_port_style"] = serde_json::Value::String(token.to_string());
    })
}

/// Read `label_style` from preferences file.
/// Defaults to `Standard` when missing or invalid.
pub fn read_label_style_pref() -> LabelStyle {
    read_label_style_pref_at(&prefs_path())
}

pub fn read_label_style_pref_at(path: &Path) -> LabelStyle {
    let raw = read_prefs_json(path)
        .and_then(|j| j["label_style"].as_str().map(str::to_string))
        .unwrap_or_default();
    if raw.eq_ignore_ascii_case("altium") {
        LabelStyle::Altium
    } else {
        LabelStyle::Standard
    }
}

/// Persist label style without clobbering other preference keys.
pub fn write_label_style_pref(style: LabelStyle) {
    write_label_style_pref_at(&prefs_path(), style)
}

pub fn write_label_style_pref_at(path: &Path, style: LabelStyle) {
    let token = match style {
        LabelStyle::Standard => "standard",
        LabelStyle::Altium => "altium",
    };
    update_prefs_json(path, |json| {
        json["label_style"] = serde_json::Value::String(token.to_string());
    })
}

/// Read `multisheet_style` from preferences file.
/// Defaults to `Standard` when missing or invalid.
pub fn read_multisheet_style_pref() -> MultisheetStyle {
    read_multisheet_style_pref_at(&prefs_path())
}

pub fn read_multisheet_style_pref_at(path: &Path) -> MultisheetStyle {
    let raw = read_prefs_json(path)
        .and_then(|j| j["multisheet_style"].as_str().map(str::to_string))
        .unwrap_or_default();
    if raw.eq_ignore_ascii_case("altium") {
        MultisheetStyle::Altium
    } else {
        MultisheetStyle::Standard
    }
}

/// Persist multisheet style without clobbering other preference keys.
pub fn write_multisheet_style_pref(style: MultisheetStyle) {
    write_multisheet_style_pref_at(&prefs_path(), style)
}

pub fn write_multisheet_style_pref_at(path: &Path, style: MultisheetStyle) {
    let token = match style {
        MultisheetStyle::Standard => "standard",
        MultisheetStyle::Altium => "altium",
    };
    update_prefs_json(path, |json| {
        json["multisheet_style"] = serde_json::Value::String(token.to_string());
    })
}

/// Read the schematic visible-grid `grid_style` preference. Defaults
/// to `Dots` (matches the previous hard-coded behaviour).
pub fn read_grid_style_pref() -> GridStyle {
    read_grid_style_pref_at(&prefs_path())
}

pub fn read_grid_style_pref_at(path: &Path) -> GridStyle {
    let raw = read_prefs_json(path)
        .and_then(|j| j["grid_style"].as_str().map(str::to_string))
        .unwrap_or_default();
    if raw.eq_ignore_ascii_case("lines") {
        GridStyle::Lines
    } else if raw.eq_ignore_ascii_case("crosses")
        || raw.eq_ignore_ascii_case("small_crosses")
        || raw.eq_ignore_ascii_case("smallcrosses")
    {
        GridStyle::SmallCrosses
    } else {
        GridStyle::Dots
    }
}

/// Persist grid style without clobbering other preference keys.
pub fn write_grid_style_pref(style: GridStyle) {
    write_grid_style_pref_at(&prefs_path(), style)
}

pub fn write_grid_style_pref_at(path: &Path, style: GridStyle) {
    let token = match style {
        GridStyle::Dots => "dots",
        GridStyle::Lines => "lines",
        GridStyle::SmallCrosses => "crosses",
    };
    update_prefs_json(path, |json| {
        json["grid_style"] = serde_json::Value::String(token.to_string());
    })
}

// ──────────────────────────────────────────────────────────────────────────
// Session-state prefs (UX_IMPROVEMENTS_OVER_ALTIUM §1.5)
// ──────────────────────────────────────────────────────────────────────────
//
// Each user-toggleable knob persists across sessions so the editor
// reopens to the state the user left it in. Reads return safe defaults
// when the prefs file is absent or the key missing — the same as a
// fresh install.

// ──────────────────────────────────────────────────────────────────────
// Generic prefs helpers (testability + dedup).
// ──────────────────────────────────────────────────────────────────────

/// Read `prefs.json` at `path` and parse to JSON value. Returns `None`
/// when the file is absent OR malformed — same semantics every read
/// pref needs ("treat as missing, use default").
fn read_prefs_json(path: &Path) -> Option<serde_json::Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
}

/// Update one key of `prefs.json` at `path` without clobbering other
/// keys. Creates the parent dir if missing. I/O failures are
/// best-effort but logged at `debug` (MD-32) — set
/// `RUST_LOG=signex_app::fonts=debug` to see them.
fn update_prefs_json(path: &Path, mut mutator: impl FnMut(&mut serde_json::Value)) {
    let mut json: serde_json::Value = std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    mutator(&mut json);
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(path, serialized.as_bytes(), "update_prefs_json");
    }
}

// ──────────────────────────────────────────────────────────────────────
// Theme
// ──────────────────────────────────────────────────────────────────────

/// Read the last-applied theme. Defaults to `ThemeId::Signex`.
pub fn read_theme_pref() -> ThemeId {
    read_theme_pref_at(&prefs_path())
}

/// Same as [`read_theme_pref`] but reads from `path` — exposed for
/// integration tests that inject a tempdir prefs file.
pub fn read_theme_pref_at(path: &Path) -> ThemeId {
    read_prefs_json(path)
        .and_then(|json| json.get("theme").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(ThemeId::Signex)
}

/// Persist theme without clobbering other preference keys.
pub fn write_theme_pref(theme: ThemeId) {
    write_theme_pref_at(&prefs_path(), theme)
}

/// Same as [`write_theme_pref`] but writes to `path` — exposed for tests.
pub fn write_theme_pref_at(path: &Path, theme: ThemeId) {
    update_prefs_json(path, |json| {
        if let Ok(value) = serde_json::to_value(theme) {
            json["theme"] = value;
        }
    })
}

// ──────────────────────────────────────────────────────────────────────
// Unit
// ──────────────────────────────────────────────────────────────────────

/// Read the last-active coordinate unit. Defaults to `Unit::Mm`.
pub fn read_unit_pref() -> Unit {
    read_unit_pref_at(&prefs_path())
}

pub fn read_unit_pref_at(path: &Path) -> Unit {
    read_prefs_json(path)
        .and_then(|json| json.get("unit").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(Unit::Mm)
}

/// Persist coordinate unit without clobbering other preference keys.
pub fn write_unit_pref(unit: Unit) {
    write_unit_pref_at(&prefs_path(), unit)
}

pub fn write_unit_pref_at(path: &Path, unit: Unit) {
    update_prefs_json(path, |json| {
        if let Ok(value) = serde_json::to_value(unit) {
            json["unit"] = value;
        }
    })
}

// ──────────────────────────────────────────────────────────────────────
// Grid visible
// ──────────────────────────────────────────────────────────────────────

/// Read the last grid-visible toggle. Defaults to `true`.
pub fn read_grid_visible_pref() -> bool {
    read_grid_visible_pref_at(&prefs_path())
}

pub fn read_grid_visible_pref_at(path: &Path) -> bool {
    read_prefs_json(path)
        .and_then(|json| json["grid_visible"].as_bool())
        .unwrap_or(true)
}

pub fn write_grid_visible_pref(visible: bool) {
    write_grid_visible_pref_at(&prefs_path(), visible)
}

pub fn write_grid_visible_pref_at(path: &Path, visible: bool) {
    update_prefs_json(path, |json| {
        json["grid_visible"] = serde_json::Value::Bool(visible);
    })
}

// ──────────────────────────────────────────────────────────────────────
// Snap enabled
// ──────────────────────────────────────────────────────────────────────

/// Read the last snap-enabled toggle. Defaults to `true`.
pub fn read_snap_enabled_pref() -> bool {
    read_snap_enabled_pref_at(&prefs_path())
}

pub fn read_snap_enabled_pref_at(path: &Path) -> bool {
    read_prefs_json(path)
        .and_then(|json| json["snap_enabled"].as_bool())
        .unwrap_or(true)
}

pub fn write_snap_enabled_pref(enabled: bool) {
    write_snap_enabled_pref_at(&prefs_path(), enabled)
}

pub fn write_snap_enabled_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, |json| {
        json["snap_enabled"] = serde_json::Value::Bool(enabled);
    })
}

// ──────────────────────────────────────────────────────────────────────
// Grid size (mm)
// ──────────────────────────────────────────────────────────────────────

/// Read the last grid size (mm). Returns `None` when missing so the
/// caller can fall back to the engine's preferred default.
pub fn read_grid_size_mm_pref() -> Option<f32> {
    read_grid_size_mm_pref_at(&prefs_path())
}

pub fn read_grid_size_mm_pref_at(path: &Path) -> Option<f32> {
    read_prefs_json(path).and_then(|json| json["grid_size_mm"].as_f64().map(|v| v as f32))
}

pub fn write_grid_size_mm_pref(grid_size_mm: f32) {
    write_grid_size_mm_pref_at(&prefs_path(), grid_size_mm)
}

pub fn write_grid_size_mm_pref_at(path: &Path, grid_size_mm: f32) {
    update_prefs_json(path, |json| {
        json["grid_size_mm"] = serde_json::json!(grid_size_mm);
    })
}

/// Read ERC severity overrides from preferences file. Returns an empty
/// map if the file is absent or the key missing — callers treat "no
/// entry" as "use the rule's default severity", matching the ui_state
/// semantic used throughout the app.
pub fn read_erc_severity_overrides()
-> std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity> {
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return std::collections::HashMap::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return std::collections::HashMap::new();
    };
    let Some(obj) = json.get("erc_severity").and_then(|v| v.as_object()) else {
        return std::collections::HashMap::new();
    };
    let mut out = std::collections::HashMap::new();
    for (rule_key, sev_value) in obj {
        let Some(rule) = parse_erc_rule_kind(rule_key) else {
            continue;
        };
        let Some(sev) = sev_value.as_str().and_then(parse_erc_severity) else {
            continue;
        };
        out.insert(rule, sev);
    }
    out
}

/// Persist the ERC severity-override map. Stored as an object keyed by
/// rule name so the file stays human-readable when the user edits it by
/// hand.
pub fn write_erc_severity_overrides(
    overrides: &std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) {
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));

    let mut obj = serde_json::Map::new();
    for (rule, sev) in overrides {
        obj.insert(
            erc_rule_kind_key(*rule).to_string(),
            serde_json::Value::String(erc_severity_key(*sev).to_string()),
        );
    }
    json["erc_severity"] = serde_json::Value::Object(obj);

    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "erc_severity_overrides");
    }
}

fn erc_rule_kind_key(rule: signex_erc::RuleKind) -> &'static str {
    use signex_erc::RuleKind::*;
    match rule {
        UnusedPin => "unused_pin",
        DuplicateRefDesignator => "duplicate_ref_designator",
        HierPortDisconnected => "hier_port_disconnected",
        DanglingWire => "dangling_wire",
        NetLabelConflict => "net_label_conflict",
        OrphanLabel => "orphan_label",
        BusBitWidthMismatch => "bus_bit_width_mismatch",
        BadHierSheetPin => "bad_hier_sheet_pin",
        MissingPowerFlag => "missing_power_flag",
        PowerPortShort => "power_port_short",
        SymbolOutsideSheet => "symbol_outside_sheet",
    }
}

fn parse_erc_rule_kind(s: &str) -> Option<signex_erc::RuleKind> {
    use signex_erc::RuleKind::*;
    Some(match s {
        "unused_pin" => UnusedPin,
        "duplicate_ref_designator" => DuplicateRefDesignator,
        "hier_port_disconnected" => HierPortDisconnected,
        "dangling_wire" => DanglingWire,
        "net_label_conflict" => NetLabelConflict,
        "orphan_label" => OrphanLabel,
        "bus_bit_width_mismatch" => BusBitWidthMismatch,
        "bad_hier_sheet_pin" => BadHierSheetPin,
        "missing_power_flag" => MissingPowerFlag,
        "power_port_short" => PowerPortShort,
        "symbol_outside_sheet" => SymbolOutsideSheet,
        _ => return None,
    })
}

fn erc_severity_key(sev: signex_erc::Severity) -> &'static str {
    match sev {
        signex_erc::Severity::Error => "error",
        signex_erc::Severity::Warning => "warning",
        signex_erc::Severity::Info => "info",
        signex_erc::Severity::Off => "off",
    }
}

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

/// Persist the list of dock panels per region + their active index
/// so the next session reopens with the same layout.
pub fn write_dock_layout(dock: &crate::dock::DockArea) {
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));

    fn region_to_json(
        dock: &crate::dock::DockArea,
        pos: crate::dock::PanelPosition,
    ) -> serde_json::Value {
        let kinds: Vec<&str> = dock
            .panel_kinds(pos)
            .iter()
            .map(|k| panel_kind_key(*k))
            .collect();
        serde_json::json!({
            "panels": kinds,
            "collapsed": dock.is_collapsed(pos),
        })
    }

    json["dock"] = serde_json::json!({
        "left":   region_to_json(dock, crate::dock::PanelPosition::Left),
        "right":  region_to_json(dock, crate::dock::PanelPosition::Right),
        "bottom": region_to_json(dock, crate::dock::PanelPosition::Bottom),
    });

    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "fonts_pref");
    }
}

/// Rebuild a DockArea from persisted JSON. Returns None when no saved
/// layout exists so the caller can fall back to the default seed.
pub fn read_dock_layout() -> Option<crate::dock::DockArea> {
    let path = prefs_path();
    let bytes = std::fs::read(&path).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let dock_v = json.get("dock")?.as_object()?;
    let mut dock = crate::dock::DockArea::new();
    for (region_key, pos) in [
        ("left", crate::dock::PanelPosition::Left),
        ("right", crate::dock::PanelPosition::Right),
        ("bottom", crate::dock::PanelPosition::Bottom),
    ] {
        let Some(region) = dock_v.get(region_key) else {
            continue;
        };
        if let Some(arr) = region.get("panels").and_then(|v| v.as_array()) {
            for val in arr {
                if let Some(s) = val.as_str()
                    && let Some(kind) = parse_panel_kind(s)
                {
                    dock.add_panel(pos, kind);
                }
            }
        }
        // Collapsed flag: run ToggleRegion once via the dock API so
        // the mutation path stays authoritative.
        if let Some(c) = region.get("collapsed").and_then(|v| v.as_bool())
            && c
        {
            dock.update(crate::dock::DockMessage::ToggleCollapse(pos));
        }
    }
    Some(dock)
}

fn panel_kind_key(k: crate::panels::PanelKind) -> &'static str {
    use crate::panels::PanelKind::*;
    match k {
        Projects => "projects",
        Components => "components",
        Navigator => "navigator",
        Properties => "properties",
        Filter => "filter",
        Erc => "erc",
        SchFilter => "sch_filter",
        SchList => "sch_list",
        Messages => "messages",
        Signal => "signal",
        Drc => "drc",
        BomStudio => "bom_studio",
        Favorites => "favorites",
        Snippets => "snippets",
        Variants => "variants",
        OutputJobs => "output_jobs",
        Todo => "todo",
        Wiki => "wiki",
        LayerStack => "layer_stack",
        NetClasses => "net_classes",
        Library => "library",
        SchLibrary => "sch_library",
        FootprintLibrary => "footprint_library",
        History => "history",
    }
}

fn parse_panel_kind(s: &str) -> Option<crate::panels::PanelKind> {
    use crate::panels::PanelKind::*;
    Some(match s {
        "projects" => Projects,
        "components" => Components,
        "navigator" => Navigator,
        "properties" => Properties,
        "filter" => Filter,
        "erc" => Erc,
        "sch_filter" => SchFilter,
        "sch_list" => SchList,
        "messages" => Messages,
        "signal" => Signal,
        "drc" => Drc,
        "bom_studio" => BomStudio,
        "favorites" => Favorites,
        "snippets" => Snippets,
        "variants" => Variants,
        "output_jobs" => OutputJobs,
        "todo" => Todo,
        "wiki" => Wiki,
        "layer_stack" => LayerStack,
        "net_classes" => NetClasses,
        "library" => Library,
        "sch_library" => SchLibrary,
        "footprint_library" => FootprintLibrary,
        "history" => History,
        _ => return None,
    })
}

fn parse_erc_severity(s: &str) -> Option<signex_erc::Severity> {
    Some(match s {
        "error" => signex_erc::Severity::Error,
        "warning" => signex_erc::Severity::Warning,
        "info" => signex_erc::Severity::Info,
        "off" => signex_erc::Severity::Off,
        _ => return None,
    })
}

/// Read the pin-connection matrix overrides. Keys stored as `"row,col"`
/// strings and values as the same severity strings as ERC overrides.
pub fn read_pin_matrix_overrides() -> std::collections::HashMap<(u8, u8), signex_erc::Severity> {
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return std::collections::HashMap::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return std::collections::HashMap::new();
    };
    let Some(obj) = json.get("pin_matrix").and_then(|v| v.as_object()) else {
        return std::collections::HashMap::new();
    };
    let mut out = std::collections::HashMap::new();
    for (k, v) in obj {
        let Some((r, c)) = k.split_once(',') else {
            continue;
        };
        let Ok(r) = r.parse::<u8>() else {
            continue;
        };
        let Ok(c) = c.parse::<u8>() else {
            continue;
        };
        let Some(sev) = v.as_str().and_then(parse_erc_severity) else {
            continue;
        };
        out.insert((r, c), sev);
    }
    out
}

/// Persist pin-connection matrix overrides.
pub fn write_pin_matrix_overrides(
    overrides: &std::collections::HashMap<(u8, u8), signex_erc::Severity>,
) {
    let path = prefs_path();
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));
    let mut obj = serde_json::Map::new();
    for ((r, c), sev) in overrides {
        obj.insert(
            format!("{r},{c}"),
            serde_json::Value::String(erc_severity_key(*sev).to_string()),
        );
    }
    json["pin_matrix"] = serde_json::Value::Object(obj);
    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        write_pref_atomic(&path, serialized.as_bytes(), "fonts_pref");
    }
}

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
