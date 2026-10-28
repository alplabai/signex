//! ERC severity / pin-matrix preference IO. Split from `fonts.rs`.

use super::*;

/// Read ERC severity overrides from preferences file. Returns an empty
/// map if the file is absent or the key missing — callers treat "no
/// entry" as "use the rule's default severity", matching the ui_state
/// semantic used throughout the app.
pub fn read_erc_severity_overrides()
-> std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity> {
    read_erc_severity_overrides_from(load_prefs_json().as_ref())
}

pub fn read_erc_severity_overrides_from(
    json: Option<&serde_json::Value>,
) -> std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity> {
    let Some(json) = json else {
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
    update_prefs_json(&prefs_path(), "erc_severity", |prefs| {
        let mut obj = serde_json::Map::new();
        for (rule, sev) in overrides {
            obj.insert(
                erc_rule_kind_key(*rule).to_string(),
                serde_json::Value::String(erc_severity_key(*sev).to_string()),
            );
        }
        prefs.insert("erc_severity".to_string(), serde_json::Value::Object(obj));
    })
}

fn erc_rule_kind_key(rule: signex_erc::RuleKind) -> &'static str {
    rule.key()
}

fn parse_erc_rule_kind(s: &str) -> Option<signex_erc::RuleKind> {
    signex_erc::RuleKind::from_key(s)
}

fn erc_severity_key(sev: signex_erc::Severity) -> &'static str {
    sev.key()
}

fn parse_erc_severity(s: &str) -> Option<signex_erc::Severity> {
    signex_erc::Severity::from_key(s)
}

/// Read the pin-connection matrix overrides. Keys stored as `"row,col"`
/// strings and values as the same severity strings as ERC overrides.
pub fn read_pin_matrix_overrides() -> std::collections::HashMap<(u8, u8), signex_erc::Severity> {
    read_pin_matrix_overrides_from(load_prefs_json().as_ref())
}

pub fn read_pin_matrix_overrides_from(
    json: Option<&serde_json::Value>,
) -> std::collections::HashMap<(u8, u8), signex_erc::Severity> {
    let Some(json) = json else {
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
    update_prefs_json(&prefs_path(), "pin_matrix", |prefs| {
        let mut obj = serde_json::Map::new();
        for ((r, c), sev) in overrides {
            obj.insert(
                format!("{r},{c}"),
                serde_json::Value::String(erc_severity_key(*sev).to_string()),
            );
        }
        prefs.insert("pin_matrix".to_string(), serde_json::Value::Object(obj));
    })
}
