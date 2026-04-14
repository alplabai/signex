//! Font management for Signex.
//!
//! Responsibilities:
//! - Enumerate system font families using fontdb (done once, cached).
//! - Provide the canonical canvas font constant (Iosevka).
//! - Read / write the UI font preference from a simple JSON config file.
//!
//! Config file: `~/.config/signex/prefs.json`
//! Format: `{"ui_font": "Roboto"}`

use std::path::PathBuf;
use std::sync::OnceLock;

use signex_render::PowerPortStyle;

/// Default UI font family name. Used when no preference file is found.
pub const DEFAULT_UI_FONT: &str = "Roboto";

/// Default canvas (schematic / PCB) font family name.
/// Iosevka is bundled in `assets/fonts/` and loaded at startup.
pub const DEFAULT_CANVAS_FONT: &str = "Iosevka";

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

fn prefs_path() -> PathBuf {
    // Respect XDG_CONFIG_HOME if set, otherwise use ~/.config
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
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return DEFAULT_UI_FONT.to_string();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return DEFAULT_UI_FONT.to_string();
    };
    json["ui_font"]
        .as_str()
        .unwrap_or(DEFAULT_UI_FONT)
        .to_string()
}

/// Persist the given `ui_font` choice to the preferences file.
/// Creates parent directories if they do not exist.
/// Silently ignores I/O errors (non-critical preference).
pub fn write_ui_font_pref(font_name: &str) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Read existing prefs so we don't clobber other keys.
    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));

    json["ui_font"] = serde_json::Value::String(font_name.to_string());

    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        let _ = std::fs::write(&path, serialized);
    }
}

/// Read `power_port_style` from preferences file.
/// Defaults to `Altium` when missing or invalid.
pub fn read_power_port_style_pref() -> PowerPortStyle {
    let path = prefs_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return PowerPortStyle::Altium;
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return PowerPortStyle::Altium;
    };

    match json["power_port_style"].as_str().unwrap_or("altium") {
        "kicad" | "KiCad" => PowerPortStyle::KiCad,
        _ => PowerPortStyle::Altium,
    }
}

/// Persist power port style without clobbering other preference keys.
pub fn write_power_port_style_pref(style: PowerPortStyle) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut json: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::json!({}));

    json["power_port_style"] = serde_json::Value::String(match style {
        PowerPortStyle::KiCad => "kicad".to_string(),
        PowerPortStyle::Altium => "altium".to_string(),
    });

    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
        let _ = std::fs::write(&path, serialized);
    }
}
