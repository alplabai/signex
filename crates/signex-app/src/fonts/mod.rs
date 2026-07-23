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
//!
//! Fallback: if the OS reports no config directory at all
//! (`dirs::config_dir()` returns `None` — rare, e.g. a daemon or
//! container with neither `$HOME` nor `$XDG_CONFIG_HOME`), preferences
//! resolve to a per-user subdirectory of the OS temp dir instead (see
//! [`production_temp_fallback_path`]), and a `tracing::warn!` names the
//! fact that they will not survive a temp-directory sweep.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::render_config::{
    GridStyle, LabelStyle, MultisheetStyle, PinSelectionMode, PowerPortStyle,
};
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
///
/// Under `cfg(test)` (signex-app's own unit tests) or the
/// `test-prefs-redirect` feature (activated for every crate that links
/// signex-app as a dev-dependency — see `Cargo.toml`), this resolves to
/// a per-process tempdir instead and returns *before* touching
/// `dirs::config_dir()` or `legacy_posix_prefs_path()` at all — so the
/// legacy migration never runs and never reads/writes the developer's
/// real config directory. Issue #437.
fn prefs_path() -> PathBuf {
    use std::sync::OnceLock;
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        // `cfg!(test)` is belt-and-braces, not load-bearing: the only unit
        // where it evaluates true is signex-app's own unit-test build, and
        // that build always carries `test-prefs-redirect` too (self dev-
        // dependency, see `Cargo.toml`) — so the feature check alone
        // already decides this branch. Kept in case that wiring changes.
        if cfg!(test) || cfg!(feature = "test-prefs-redirect") {
            return std::env::temp_dir()
                .join(format!("signex-test-prefs-{}", std::process::id()))
                .join("prefs.json");
        }
        let canonical = match dirs::config_dir() {
            Some(dir) => dir.join("signex").join("prefs.json"),
            None => production_temp_fallback_path(),
        };
        let legacy = legacy_posix_prefs_path();
        migrate_legacy_prefs(&canonical, &legacy);
        canonical
    })
    .clone()
}

/// Fallback used only when `dirs::config_dir()` returns `None` (rare — a
/// Windows account with no `%APPDATA%`, a daemon/container with neither
/// `$XDG_CONFIG_HOME` nor `$HOME`). Issue #437's review flagged the naive
/// version of this fallback (a bare `<tmp>/signex/prefs.json`) as
/// predictable and shared: two users on one host collide (the second gets
/// `EACCES` from `atomic_write`, visible only at `tracing::debug!`, or
/// silently reads the first user's prefs), and an attacker who pre-creates
/// `prefs.json.tmp` as a symlink gets `atomic_write`'s `File::create` to
/// write through it.
///
/// Discriminated by the OS username env var instead — the cheapest
/// per-user handle available without a new dependency. If no such var is
/// set either, there is no safe shared path left to fall back to; per the
/// issue's own desired end state ("fail loudly, rather than silently
/// landing in [a shared/CWD path]"), this panics rather than write
/// anywhere.
fn production_temp_fallback_path() -> PathBuf {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .or_else(|_| std::env::var("LOGNAME"))
        .ok()
        .filter(|s| !s.is_empty());
    let Some(user) = user else {
        panic!(
            "signex: no OS config directory (dirs::config_dir() returned None) and no \
             USER/USERNAME/LOGNAME env var to derive a per-user temp fallback from — \
             refusing to write preferences to a predictable shared path (issue #437)"
        );
    };
    tracing::warn!(
        target = "signex::prefs",
        user = %user,
        "no OS config directory found (dirs::config_dir() returned None); \
         falling back to a per-user temp directory — preferences will NOT \
         persist across a temp-directory sweep"
    );
    std::env::temp_dir()
        .join(format!("signex-{user}"))
        .join("prefs.json")
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
    // Routed through `write_pref_atomic` (temp + fsync + rename) rather
    // than `std::fs::copy`: a kill mid-copy with a bare copy can leave a
    // truncated `canonical` file, which then blocks re-migration forever
    // because `canonical.exists()` is already true on the next launch.
    if !canonical.exists() && legacy != canonical && legacy.exists() {
        if let Ok(bytes) = std::fs::read(legacy) {
            write_pref_atomic(canonical, &bytes, "migrate_legacy_prefs_copy");
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
// PCB GPU render (experimental)
// ──────────────────────────────────────────────────────────────────────

/// Read the PCB GPU-render toggle. Defaults to the compile-time
/// [`crate::feature_flags::PCB_GPU_RENDER`] when the key is absent, so the
/// const acts as the factory default and old prefs files stay compatible.
pub fn read_pcb_gpu_render_pref() -> bool {
    read_pcb_gpu_render_pref_at(&prefs_path())
}

pub fn read_pcb_gpu_render_pref_at(path: &Path) -> bool {
    read_prefs_json(path)
        .and_then(|json| json["pcb_gpu_render"].as_bool())
        .unwrap_or(crate::feature_flags::PCB_GPU_RENDER)
}

pub fn write_pcb_gpu_render_pref(enabled: bool) {
    write_pcb_gpu_render_pref_at(&prefs_path(), enabled)
}

pub fn write_pcb_gpu_render_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, |json| {
        json["pcb_gpu_render"] = serde_json::Value::Bool(enabled);
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

/// Read the default symbol-editor grid size (mm). Falls back to 1.27 mm.
pub fn read_symbol_grid_size_mm_pref() -> f32 {
    read_prefs_json(&prefs_path())
        .and_then(|json| json["symbol_grid_size_mm"].as_f64().map(|v| v as f32))
        .unwrap_or(1.27)
}

pub fn write_symbol_grid_size_mm_pref(grid_size_mm: f32) {
    update_prefs_json(&prefs_path(), |json| {
        json["symbol_grid_size_mm"] = serde_json::json!(grid_size_mm);
    })
}

/// Read the symbol-editor grid style preference. Defaults to `Dots`.
pub fn read_symbol_grid_style_pref() -> GridStyle {
    let raw = read_prefs_json(&prefs_path())
        .and_then(|j| j["symbol_grid_style"].as_str().map(str::to_string))
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

pub fn write_symbol_grid_style_pref(style: GridStyle) {
    let token = match style {
        GridStyle::Dots => "dots",
        GridStyle::Lines => "lines",
        GridStyle::SmallCrosses => "crosses",
    };
    update_prefs_json(&prefs_path(), |json| {
        json["symbol_grid_style"] = serde_json::Value::String(token.to_string());
    })
}

/// Read the symbol-editor pin-selection preference. Defaults to `PinOnly`.
pub fn read_symbol_pin_selection_pref() -> PinSelectionMode {
    let raw = read_prefs_json(&prefs_path())
        .and_then(|j| j["symbol_pin_selection"].as_str().map(str::to_string))
        .unwrap_or_default();
    PinSelectionMode::from_pref_token(&raw)
}

pub fn write_symbol_pin_selection_pref(mode: PinSelectionMode) {
    update_prefs_json(&prefs_path(), |json| {
        json["symbol_pin_selection"] = serde_json::Value::String(mode.pref_token().to_string());
    })
}

mod dock_layout;
mod erc;
mod misc;
mod presets;

pub use dock_layout::*;
pub use erc::*;
pub use misc::*;
pub use presets::*;
