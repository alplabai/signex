//! ERC severity / pin-matrix preference IO. Split from `fonts.rs`.

use super::*;

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

