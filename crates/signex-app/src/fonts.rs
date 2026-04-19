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
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

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
        let _ = std::fs::write(&path, serialized);
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

/// Persist the list of dock panels per region + their active index
/// so the next session reopens with the same layout.
pub fn write_dock_layout(dock: &crate::dock::DockArea) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
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
        let _ = std::fs::write(&path, serialized);
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
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
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
        let _ = std::fs::write(&path, serialized);
    }
}
