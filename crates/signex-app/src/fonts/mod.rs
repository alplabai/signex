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
//! Format: a JSON object holding `ui_font` plus the other persisted keys
//! (e.g. `component_classes`); writers preserve keys they do not own.
//!
//! Fallback: if the OS reports no config directory at all
//! (`dirs::config_dir()` returns `None` — rare, e.g. a daemon or
//! container with neither `$HOME` nor `$XDG_CONFIG_HOME`), preferences
//! resolve to a random-named per-process subdirectory of the OS temp
//! dir instead (see [`production_temp_fallback_path`]), and a
//! `tracing::error!` names the fact that they will not survive a
//! temp-directory sweep or a restart.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::render_config::{GridStyle, PinSelectionMode};
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

/// Default UI font family name. Used when no preference file is found.
pub const DEFAULT_UI_FONT: &str = "Inter";

/// Bundled monospace family for chrome-only mono text (shortcuts, values).
/// Not a canvas font: `signex_types::font::BUNDLED_DEFAULT_FAMILY` stays Iosevka.
pub const CHROME_MONO_FONT: &str = "JetBrains Mono";

/// MD-32: persist `bytes` to `path` atomically (tmp + rename via
/// `signex_types::atomic_io`) and report on failure, instead of the
/// `let _ = std::fs::write(...)` pattern that swallows disk-full /
/// permission errors silently. Every preferences write in this module
/// tree funnels through here, so this is the single place a failed write
/// can be seen from.
///
/// Reported at `Error` and not `debug!`: the default filter is
/// `LevelFilter::Info` (`crate::diagnostics`), so a `debug!` here never
/// reaches the user's Messages panel and the failure is invisible in
/// every shipped build — which is the whole class of bug #594 and #602
/// were about. The setting the user just changed did not persist and
/// will be gone at the next launch; that is not a `debug!`.
fn write_pref_atomic(path: &Path, bytes: &[u8], context: &str) -> bool {
    if let Err(e) = signex_types::atomic_io::atomic_write(path, bytes) {
        tracing::error!(
            target: "signex::prefs",
            path = %path.display(),
            context = context,
            error = %e,
            "a preference was not saved: writing the preferences file failed, so this \
             change will be gone at the next launch — the setting itself is still \
             applied for this session"
        );
        return false;
    }
    true
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

/// Build an [`iced::Font`] that targets the given family name. The Preferences
/// panel hands us font names as runtime `String`s; iced's `Family::name`
/// interns them in a global cache (one leaked `&'static str` per unique
/// name, bounded by the small set of fonts a user actually picks). Used for
/// surfaces that need the canvas font (Iosevka by default) — e.g. the symbol
/// hover tooltip.
pub fn iced_font_for_family(name: &str) -> iced::Font {
    iced::Font::with_family(iced::font::Family::name(name))
}

/// The font canvas text is drawn in: the bundled Iosevka for the default or
/// empty setting, the named family otherwise. Pair with
/// [`signex_types::font::nonblank_family`] so a widget measures in the face it draws.
pub fn canvas_iced_font(name: &str) -> iced::Font {
    match signex_types::font::nonblank_family(name) {
        None | Some(DEFAULT_CANVAS_FONT) => crate::render_config::IOSEVKA,
        Some(family) => iced_font_for_family(family),
    }
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
    // Same scan the renderer reads (`catalog`), so the font pickers list
    // exactly the families a document can resolve to. The bundled families
    // are always in it, Iosevka included.
    CACHE.get_or_init(|| catalog::installed_blocking().names().to_vec())
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
/// The directory itself comes from [`crate::config_root::config_root`],
/// shared by the other three config-file resolvers. Under
/// `cfg(test)`/`test-prefs-redirect` that resolves to a per-process
/// tempdir, and this function returns *before* touching
/// `legacy_posix_prefs_path()` at all — so the legacy migration never
/// runs and never reads/writes the developer's real config directory.
/// Issue #437, hoisted in #440. A headless config root skips it the same
/// way (#1926).
fn prefs_path() -> PathBuf {
    use std::sync::OnceLock;
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let root = crate::config_root::config_root();
        let may_migrate = may_migrate_legacy_prefs(
            crate::config_root::is_test_redirect_active(),
            crate::config_root::is_override_active(),
        );
        if !may_migrate {
            // Branch on the gates themselves, not on whether `root` happens
            // to be `Some`: a `Some(dir) if gate` match guard would fall
            // through to the migration below if `config_root()` ever
            // returned `None` here, and run it against the developer's or
            // the user's real legacy path (#440 review, #1926).
            let dir =
                root.expect("config_root() always resolves under the redirect or an override");
            return dir.join("prefs.json");
        }
        let canonical = match root {
            Some(dir) => dir.join("prefs.json"),
            None => production_temp_fallback_path(),
        };
        let legacy = legacy_posix_prefs_path();
        migrate_legacy_prefs(&canonical, &legacy);
        canonical
    })
    .clone()
}

/// Whether the one-shot legacy-prefs migration may run: never in a test
/// build, never under a headless config root (#1926). Both point the prefs
/// at a throwaway folder, and the migration reads and rewrites the real one.
fn may_migrate_legacy_prefs(is_test_redirect: bool, is_override: bool) -> bool {
    !is_test_redirect && !is_override
}

/// Fallback used only when `dirs::config_dir()` returns `None` (rare — a
/// Windows account with no `%APPDATA%`, a daemon/container with neither
/// `$XDG_CONFIG_HOME` nor `$HOME`). Issue #437's review flagged the naive
/// version of this fallback (a bare `<tmp>/signex/prefs.json`) as
/// predictable and shared: two users on one host collide (the second gets
/// `EACCES` from `atomic_write`, reported at `tracing::error!`, or
/// silently reads the first user's prefs), and an attacker who pre-creates
/// `prefs.json.tmp` as a symlink gets `atomic_write`'s `File::create` to
/// write through it.
///
/// A later per-user `<tmp>/signex-{user}` scheme fixed the sharing
/// problem but leaned on `USER`/`USERNAME`/`LOGNAME` being set — a
/// bare-uid container or a systemd unit with a scrubbed environment has
/// none of those, and that version `panic!`ed there, taking the whole
/// app down before the window ever appeared (#440 review: "fail loudly"
/// was meant to mean a visible log line, not a vanished process).
///
/// `tempfile::Builder::tempdir()` replaces that scheme outright: the
/// directory name is process-random (nothing to pre-guess for the
/// symlink attack above, no username needed) and it's created 0700 on
/// Unix — strictly safer than the username-keyed scheme even on the
/// happy path. If even creating a temp directory fails (the temp
/// filesystem itself is unwritable — every other option has already
/// failed too), this hands back a path anyway rather than panicking:
/// every write through it fails at `atomic_write`'s own
/// `tracing::error!`, and the app simply runs the session on in-memory
/// defaults. Degrade, don't die.
fn production_temp_fallback_path() -> PathBuf {
    let dir = match tempfile::Builder::new().prefix("signex-").tempdir() {
        Ok(dir) => {
            // Leak the `TempDir` handle rather than let it delete the
            // directory when dropped at the end of this function:
            // `prefs_path()` caches the returned path for the whole
            // process lifetime and writes to it repeatedly across the
            // session, so the directory has to outlive any handle we
            // could hold instead. `keep()` is `tempfile`'s
            // documented opt-out of the drop-time cleanup; the OS's
            // own temp-dir sweep remains the intended eventual
            // cleanup, matching the "will NOT persist" contract below.
            dir.keep()
        }
        Err(e) => {
            tracing::error!(
                target: "signex::prefs",
                error = %e,
                "no OS config directory (dirs::config_dir() returned None) and \
                 creating a random per-process temp directory also failed; \
                 preferences will not persist for this session"
            );
            return std::env::temp_dir()
                .join("signex-prefs-unavailable")
                .join("prefs.json");
        }
    };
    tracing::error!(
        target: "signex::prefs",
        path = %dir.display(),
        "no OS config directory found (dirs::config_dir() returned None); \
         falling back to a random-named per-process temp directory — \
         preferences will NOT persist across a temp-directory sweep or a restart"
    );
    dir.join("prefs.json")
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
/// `legacy` is the legacy (pre-v0.12) POSIX-shaped prefs path to
/// pull from when canonical is missing. Production passes
/// [`legacy_posix_prefs_path()`]; tests inject a tempdir-shaped path.
pub fn migrate_legacy_prefs(canonical: &Path, legacy: &Path) {
    // Both steps below are read-modify-write on `canonical`.
    let _io = prefs_file::prefs_io_lock();
    // F1: copy legacy path → canonical, but only if canonical is empty.
    // Routed through `write_pref_atomic` (temp + fsync + rename) rather
    // than `std::fs::copy`: a kill mid-copy with a bare copy can leave a
    // truncated `canonical` file, which then blocks re-migration forever
    // because `canonical.exists()` is already true on the next launch.
    if !canonical.exists()
        && legacy != canonical
        && legacy.exists()
        && let Ok(bytes) = std::fs::read(legacy)
    {
        write_pref_atomic(canonical, &bytes, "migrate_legacy_prefs_copy");
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
    read_ui_font_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_ui_font_pref_from(json: Option<&serde_json::Value>) -> String {
    json.and_then(|j| j["ui_font"].as_str().map(str::to_string))
        .unwrap_or_else(|| DEFAULT_UI_FONT.to_string())
}

/// Persist the given `ui_font` choice to the preferences file.
/// Creates parent directories if they do not exist.
/// An I/O failure is reported at `tracing::error!` (see `write_pref_atomic`);
/// the setting still applies for the session.
pub fn write_ui_font_pref(font_name: &str) {
    write_ui_font_pref_at(&prefs_path(), font_name)
}

pub fn write_ui_font_pref_at(path: &Path, font_name: &str) {
    update_prefs_json(path, "ui_font", |prefs| {
        prefs.insert(
            "ui_font".to_string(),
            serde_json::Value::String(font_name.to_string()),
        );
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
    read_component_classes_pref_from(load_prefs_json().as_ref())
}

pub fn read_component_classes_pref_from(
    json: Option<&serde_json::Value>,
) -> Vec<ComponentClassEntry> {
    let Some(json) = json else {
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
/// without clobbering other preference keys. An I/O failure is reported at
/// `tracing::error!`; the in-memory setting still applies.
pub fn write_component_classes_pref(classes: &[ComponentClassEntry]) {
    update_prefs_json(&prefs_path(), "component_classes", |prefs| {
        if let Ok(value) = serde_json::to_value(classes) {
            prefs.insert("component_classes".to_string(), value);
        }
    })
}

/// Read the schematic visible-grid `grid_style` preference. Defaults
/// to `Dots` (matches the previous hard-coded behaviour).
pub fn read_grid_style_pref() -> GridStyle {
    read_grid_style_pref_at(&prefs_path())
}

pub fn read_grid_style_pref_at(path: &Path) -> GridStyle {
    read_grid_style_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_grid_style_pref_from(json: Option<&serde_json::Value>) -> GridStyle {
    let raw = json
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
    update_prefs_json(path, "grid_style", |prefs| {
        prefs.insert(
            "grid_style".to_string(),
            serde_json::Value::String(token.to_string()),
        );
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
pub fn load_prefs_json() -> Option<serde_json::Value> {
    read_prefs_json(&prefs_path())
}

fn read_prefs_json(path: &Path) -> Option<serde_json::Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
}

// ──────────────────────────────────────────────────────────────────────
// Theme
// ──────────────────────────────────────────────────────────────────────

/// Read the last-applied theme. Defaults to [`ThemeId::DEFAULT`] when none
/// is stored or the stored name is unknown (e.g. written by a newer build);
/// the other preferences are read separately and stay intact.
pub fn read_theme_pref() -> ThemeId {
    read_theme_pref_at(&prefs_path())
}

/// Same as [`read_theme_pref`] but reads from `path` — exposed for
/// integration tests that inject a tempdir prefs file.
pub fn read_theme_pref_at(path: &Path) -> ThemeId {
    read_theme_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_theme_pref_from(json: Option<&serde_json::Value>) -> ThemeId {
    json.and_then(|json| json.get("theme").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(ThemeId::DEFAULT)
}

/// Persist theme without clobbering other preference keys.
pub fn write_theme_pref(theme: ThemeId) {
    write_theme_pref_at(&prefs_path(), theme)
}

/// Same as [`write_theme_pref`] but writes to `path` — exposed for tests.
pub fn write_theme_pref_at(path: &Path, theme: ThemeId) {
    update_prefs_json(path, "theme", |prefs| {
        if let Ok(value) = serde_json::to_value(theme) {
            prefs.insert("theme".to_string(), value);
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
    read_unit_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_unit_pref_from(json: Option<&serde_json::Value>) -> Unit {
    json.and_then(|json| json.get("unit").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(Unit::Mm)
}

/// Persist coordinate unit without clobbering other preference keys.
pub fn write_unit_pref(unit: Unit) {
    write_unit_pref_at(&prefs_path(), unit)
}

pub fn write_unit_pref_at(path: &Path, unit: Unit) {
    update_prefs_json(path, "unit", |prefs| {
        if let Ok(value) = serde_json::to_value(unit) {
            prefs.insert("unit".to_string(), value);
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
    read_grid_visible_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_grid_visible_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["grid_visible"].as_bool())
        .unwrap_or(true)
}

pub fn write_grid_visible_pref(visible: bool) {
    write_grid_visible_pref_at(&prefs_path(), visible)
}

pub fn write_grid_visible_pref_at(path: &Path, visible: bool) {
    update_prefs_json(path, "grid_visible", |prefs| {
        prefs.insert("grid_visible".to_string(), serde_json::Value::Bool(visible));
    })
}

// ──────────────────────────────────────────────────────────────────────
// Anti-aliasing (MSAA 4x on canvas meshes)
// ──────────────────────────────────────────────────────────────────────

/// Read the anti-aliasing toggle. On unless the file says otherwise, so an
/// old `prefs.json` without the key, a missing file and a malformed value all
/// read as on. iced fixes it when the window system starts, so a change
/// applies after a restart.
pub fn read_antialiasing_pref() -> bool {
    read_antialiasing_pref_at(&prefs_path())
}

pub fn read_antialiasing_pref_at(path: &Path) -> bool {
    read_antialiasing_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_antialiasing_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["antialiasing"].as_bool())
        .unwrap_or(true)
}

pub fn write_antialiasing_pref(enabled: bool) {
    write_antialiasing_pref_at(&prefs_path(), enabled)
}

pub fn write_antialiasing_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, "antialiasing", |prefs| {
        prefs.insert("antialiasing".to_string(), serde_json::Value::Bool(enabled));
    })
}

// ──────────────────────────────────────────────────────────────────────
// PCB GPU render (experimental)
// ──────────────────────────────────────────────────────────────────────

/// Read the PCB GPU-render toggle. A saved value always wins; the
/// compile-time [`crate::feature_flags::PCB_GPU_RENDER_DEFAULT`] applies only
/// when the key is absent, so it is a factory default rather than a gate and
/// old prefs files stay compatible.
pub fn read_pcb_gpu_render_pref() -> bool {
    read_pcb_gpu_render_pref_at(&prefs_path())
}

pub fn read_pcb_gpu_render_pref_at(path: &Path) -> bool {
    read_pcb_gpu_render_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_pcb_gpu_render_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["pcb_gpu_render"].as_bool())
        .unwrap_or(crate::feature_flags::PCB_GPU_RENDER_DEFAULT)
}

pub fn write_pcb_gpu_render_pref(enabled: bool) {
    write_pcb_gpu_render_pref_at(&prefs_path(), enabled)
}

pub fn write_pcb_gpu_render_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, "pcb_gpu_render", |prefs| {
        prefs.insert(
            "pcb_gpu_render".to_string(),
            serde_json::Value::Bool(enabled),
        );
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
    read_snap_enabled_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_snap_enabled_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json["snap_enabled"].as_bool())
        .unwrap_or(true)
}

pub fn write_snap_enabled_pref(enabled: bool) {
    write_snap_enabled_pref_at(&prefs_path(), enabled)
}

pub fn write_snap_enabled_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, "snap_enabled", |prefs| {
        prefs.insert("snap_enabled".to_string(), serde_json::Value::Bool(enabled));
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
    read_grid_size_mm_pref_from(read_prefs_json(path).as_ref())
}

pub fn read_grid_size_mm_pref_from(json: Option<&serde_json::Value>) -> Option<f32> {
    json.and_then(|json| json["grid_size_mm"].as_f64().map(|v| v as f32))
}

pub fn write_grid_size_mm_pref(grid_size_mm: f32) {
    write_grid_size_mm_pref_at(&prefs_path(), grid_size_mm)
}

pub fn write_grid_size_mm_pref_at(path: &Path, grid_size_mm: f32) {
    update_prefs_json(path, "grid_size_mm", |prefs| {
        prefs.insert("grid_size_mm".to_string(), serde_json::json!(grid_size_mm));
    })
}

/// Read the default symbol-editor grid size (mm). Falls back to 1.27 mm.
pub fn read_symbol_grid_size_mm_pref() -> f32 {
    read_symbol_grid_size_mm_pref_from(load_prefs_json().as_ref())
}

pub fn read_symbol_grid_size_mm_pref_from(json: Option<&serde_json::Value>) -> f32 {
    json.and_then(|json| json["symbol_grid_size_mm"].as_f64().map(|v| v as f32))
        .unwrap_or(1.27)
}

pub fn write_symbol_grid_size_mm_pref(grid_size_mm: f32) {
    update_prefs_json(&prefs_path(), "symbol_grid_size_mm", |prefs| {
        prefs.insert(
            "symbol_grid_size_mm".to_string(),
            serde_json::json!(grid_size_mm),
        );
    })
}

/// Read the symbol-editor grid style preference. Defaults to `Dots`.
pub fn read_symbol_grid_style_pref() -> GridStyle {
    read_symbol_grid_style_pref_from(load_prefs_json().as_ref())
}

pub fn read_symbol_grid_style_pref_from(json: Option<&serde_json::Value>) -> GridStyle {
    let raw = json
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
    update_prefs_json(&prefs_path(), "symbol_grid_style", |prefs| {
        prefs.insert(
            "symbol_grid_style".to_string(),
            serde_json::Value::String(token.to_string()),
        );
    })
}

/// Read the symbol-editor pin-selection preference. Defaults to `PinOnly`.
pub fn read_symbol_pin_selection_pref() -> PinSelectionMode {
    read_symbol_pin_selection_pref_from(load_prefs_json().as_ref())
}

pub fn read_symbol_pin_selection_pref_from(json: Option<&serde_json::Value>) -> PinSelectionMode {
    let raw = json
        .and_then(|j| j["symbol_pin_selection"].as_str().map(str::to_string))
        .unwrap_or_default();
    PinSelectionMode::from_pref_token(&raw)
}

pub fn write_symbol_pin_selection_pref(mode: PinSelectionMode) {
    update_prefs_json(&prefs_path(), "symbol_pin_selection", |prefs| {
        prefs.insert(
            "symbol_pin_selection".to_string(),
            serde_json::Value::String(mode.pref_token().to_string()),
        );
    })
}

pub mod bundled;
pub mod catalog;
mod dock_layout;
pub mod document_report;
mod erc;
pub mod measure;
mod misc;
mod ordered_write;
#[cfg(test)]
pub(crate) use ordered_write::pref_slots_taken_on_this_thread;
mod prefs_file;
mod presets;
mod recent;
pub mod resolve;

#[cfg(test)]
pub(crate) use prefs_file::PrefsIoHold;
/// The one read-modify-write every preference writer in this module
/// tree goes through (#594). Imported here rather than re-exported —
/// the sibling modules pick it up through their own `use super::*;`.
use prefs_file::{try_update_prefs_json, update_prefs_json, update_prefs_json_checked};

/// The prefs file's health probe and its one in-app recovery (#602).
/// Re-exported because the app shell owns the reporting: `Signex::new`
/// probes at boot, the Preferences handler re-probes on open and runs the
/// recovery, and the dialog names [`prefs_file_path`] in its banner.
/// `prefs_path()` itself stays private — callers get the resolved path
/// through the accessor rather than a second resolver of their own.
pub use prefs_file::{
    PrefsLoadError, check_prefs_file, check_prefs_file_at, move_prefs_file_aside,
    move_prefs_file_aside_at, prefs_file_path,
};

pub use dock_layout::*;
pub use erc::*;
pub use misc::*;
pub use ordered_write::{OrderedPrefWrite, background_pref_write, wait_for_pref_writes};
#[cfg(test)]
pub(crate) use ordered_write::{install_wait_probe, take_wait_probe};
pub use presets::*;
pub use recent::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// #1926: only a real GUI process, with neither gate, migrates.
    #[test]
    fn the_legacy_prefs_migration_never_runs_under_a_headless_config_root() {
        assert!(!may_migrate_legacy_prefs(false, true));
        assert!(!may_migrate_legacy_prefs(true, true));
        assert!(!may_migrate_legacy_prefs(true, false));
        assert!(may_migrate_legacy_prefs(false, false));
    }

    /// A widget that measures canvas text in a family must draw in it: the
    /// font handed to the widget names the family the measurement used.
    #[test]
    fn canvas_text_is_measured_in_the_family_it_is_drawn_in() {
        for name in [DEFAULT_CANVAS_FONT, "", "  ", "Roboto Mono"] {
            let drawn = canvas_iced_font(name).family;
            let measured = signex_types::font::nonblank_family(name)
                .unwrap_or(signex_types::font::BUNDLED_DEFAULT_FAMILY);
            assert_eq!(drawn, iced::font::Family::name(measured), "{name:?}");
        }
    }

    /// `prefs_path()` is private (unlike the other three prefs
    /// resolvers, which are all `pub`), so it can't be reached from an
    /// integration test in `tests/` — this is that resolver's half of
    /// the #440 guard. Proves fonts' prefs file lands under the same
    /// `config_root()` the other three share, under the test redirect.
    #[test]
    fn prefs_path_lives_under_the_shared_config_root() {
        let root = crate::config_root::config_root().expect("test redirect always resolves");
        assert!(
            prefs_path().starts_with(&root),
            "fonts::prefs_path() must live under config_root(), got {}",
            prefs_path().display()
        );
    }

    fn temp_prefs(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::Builder::new()
            .prefix("signex-flags-")
            .tempdir()
            .expect("temp dir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, contents).expect("write prefs");
        (dir, path)
    }

    /// `PCB_GPU_RENDER_DEFAULT` is a factory default, not a gate: a saved
    /// value wins over it in both directions. The module doc in
    /// `feature_flags` used to claim the opposite, which would make someone
    /// reach for that const to disable the feature for everyone and ship a
    /// release where it was still on.
    #[test]
    fn a_saved_pcb_gpu_render_value_overrides_the_compile_time_default() {
        let (_dir, on) = temp_prefs(r#"{"pcb_gpu_render": true}"#);
        assert!(
            read_pcb_gpu_render_pref_at(&on),
            "a saved `true` must win even though the default is {}",
            crate::feature_flags::PCB_GPU_RENDER_DEFAULT
        );

        let (_dir, off) = temp_prefs(r#"{"pcb_gpu_render": false}"#);
        assert!(
            !read_pcb_gpu_render_pref_at(&off),
            "a saved `false` must win too"
        );
    }

    /// The other half of the contract: with no saved key, the const is what
    /// a fresh profile gets.
    #[test]
    fn an_absent_pcb_gpu_render_key_falls_back_to_the_compile_time_default() {
        let (_dir, empty) = temp_prefs("{}");
        assert_eq!(
            read_pcb_gpu_render_pref_at(&empty),
            crate::feature_flags::PCB_GPU_RENDER_DEFAULT
        );

        let (_dir, missing) = temp_prefs(r#"{"theme": "nord"}"#);
        assert_eq!(
            read_pcb_gpu_render_pref_at(&missing),
            crate::feature_flags::PCB_GPU_RENDER_DEFAULT,
            "an unrelated prefs file must not disturb the default"
        );
    }

    /// Anti-aliasing is a preference that defaults to on: a missing file, a
    /// file without the key and a value that is not a bool all read as on.
    #[test]
    fn an_absent_antialiasing_key_defaults_to_on() {
        let (_dir, empty) = temp_prefs("{}");
        assert!(read_antialiasing_pref_at(&empty));
        let (_dir, other) = temp_prefs(r#"{"theme": "nord"}"#);
        assert!(read_antialiasing_pref_at(&other));
        let (_dir, wrong) = temp_prefs(r#"{"antialiasing": "yes"}"#);
        assert!(read_antialiasing_pref_at(&wrong));
        assert!(read_antialiasing_pref_from(None));
    }

    #[test]
    fn a_saved_antialiasing_value_round_trips() {
        let (_dir, path) = temp_prefs(r#"{"theme": "nord"}"#);
        write_antialiasing_pref_at(&path, false);
        assert!(!read_antialiasing_pref_at(&path));
        write_antialiasing_pref_at(&path, true);
        assert!(read_antialiasing_pref_at(&path));
        let json = read_prefs_json(&path).expect("prefs");
        assert_eq!(json["theme"], "nord", "other keys are kept");
    }
}

#[cfg(test)]
mod designator_standard_tests {
    use super::DEFAULT_COMPONENT_CLASSES;
    use signex_types::designator::{DesignatorStandard, class_prefix};

    /// Every seeded class but "generic" has letters under every standard,
    /// apart from the classes EN IEC 81346-2 names no letter for, so a renamed
    /// seed key cannot silently fall back to the legacy prefix.
    #[test]
    fn every_seed_class_but_generic_resolves_under_each_standard() {
        const IEC_UNNAMED: [&str; 3] = ["comparator", "regulator_linear", "crystal"];
        for (key, _) in DEFAULT_COMPONENT_CLASSES {
            for standard in DesignatorStandard::ALL {
                let letters = class_prefix(standard, key);
                let unnamed = *key == "generic"
                    || (standard == DesignatorStandard::Iec81346_2 && IEC_UNNAMED.contains(key));
                if unnamed {
                    assert_eq!(letters, None, "{key} under {standard}");
                } else {
                    assert!(letters.is_some(), "{key} has no prefix under {standard}");
                }
            }
        }
    }
}
