//! ERC preference IO. Split from `fonts.rs`.
//!
//! The machine's ERC rules are the `User Default` rule profile now (#1608), not
//! `prefs.json` keys. The two readers below stay only for the one-time
//! migration of an old `erc_severity` / `pin_matrix` into that profile; nothing
//! writes those keys any more. `prefs.json` keeps two ERC settings: which
//! profile new projects start from, and whether live ERC runs (#1288).

use super::*;

/// Read the old ERC severity overrides from the preferences file, for the
/// migration into the `User Default` profile. Returns an empty map if the
/// file is absent or the key missing.
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

fn parse_erc_rule_kind(s: &str) -> Option<signex_erc::RuleKind> {
    signex_erc::RuleKind::from_key(s)
}

fn parse_erc_severity(s: &str) -> Option<signex_erc::Severity> {
    signex_erc::Severity::from_key(s)
}

/// Read the old pin-connection matrix overrides, for the same migration. Keys
/// were stored as `"row,col"` strings, values as severity strings.
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

/// The profile new projects start from, as `prefs.json` names it. `None`
/// when no choice was made or the value is not a non-empty string.
pub fn read_default_rule_profile_pref_from(json: Option<&serde_json::Value>) -> Option<String> {
    json?
        .get("default_rule_profile")?
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

/// Persist the default-profile choice; `None` removes the key ("automatic").
pub fn write_default_rule_profile_pref(name: Option<&str>) {
    write_default_rule_profile_pref_at(&prefs_path(), name)
}

pub fn write_default_rule_profile_pref_at(path: &Path, name: Option<&str>) {
    update_prefs_json(path, "default_rule_profile", |prefs| match name {
        Some(name) => {
            prefs.insert(
                "default_rule_profile".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }
        None => {
            prefs.remove("default_rule_profile");
        }
    })
}

/// Whether the old `erc_severity` / `pin_matrix` keys were already folded into
/// the `User Default` profile (#1608).
pub fn read_erc_rules_in_profile_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json.get("erc_rules_in_profile")?.as_bool())
        .unwrap_or(false)
}

/// Set or clear the fold marker. Returns the refusal when `prefs.json` cannot
/// be updated (an unreadable file is never overwritten).
pub fn write_erc_rules_in_profile_marker(folded: bool) -> Result<(), String> {
    write_erc_rules_in_profile_marker_at(&prefs_path(), folded)
}

pub fn write_erc_rules_in_profile_marker_at(path: &Path, folded: bool) -> Result<(), String> {
    try_update_prefs_json(path, "erc_rules_in_profile", |prefs| {
        if folded {
            prefs.insert("erc_rules_in_profile".into(), serde_json::Value::Bool(true));
        } else {
            prefs.remove("erc_rules_in_profile");
        }
    })
}

/// The `prefs.json` key of live ERC (#1288).
pub const LIVE_ERC_KEY: &str = "live_erc";

/// Read whether live ERC runs. On when the key is missing or not a boolean
/// (Caner 2026-11-25: on by default).
pub fn read_live_erc_pref_from(json: Option<&serde_json::Value>) -> bool {
    json.and_then(|json| json.get(LIVE_ERC_KEY)?.as_bool())
        .unwrap_or(true)
}

pub fn write_live_erc_pref(enabled: bool) {
    write_live_erc_pref_at(&prefs_path(), enabled)
}

pub fn write_live_erc_pref_at(path: &Path, enabled: bool) {
    update_prefs_json(path, LIVE_ERC_KEY, |prefs| {
        prefs.insert(LIVE_ERC_KEY.to_string(), serde_json::Value::Bool(enabled));
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_erc_is_on_unless_saved_off() {
        assert!(read_live_erc_pref_from(None));
        for (raw, on) in [
            (r#"{}"#, true),
            (r#"{"live_erc": "no"}"#, true),
            (r#"{"live_erc": false}"#, false),
            (r#"{"live_erc": true}"#, true),
        ] {
            let json: serde_json::Value = serde_json::from_str(raw).unwrap();
            assert_eq!(read_live_erc_pref_from(Some(&json)), on, "{raw}");
        }
    }

    #[test]
    fn the_live_erc_choice_round_trips_and_keeps_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{"theme": "nord"}"#).unwrap();
        let read = |path: &Path| {
            let json: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            (read_live_erc_pref_from(Some(&json)), json["theme"].clone())
        };
        write_live_erc_pref_at(&path, false);
        assert_eq!(read(&path), (false, serde_json::json!("nord")));
        write_live_erc_pref_at(&path, true);
        assert_eq!(read(&path), (true, serde_json::json!("nord")));
    }

    fn read_back(path: &Path) -> Option<String> {
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        read_default_rule_profile_pref_from(Some(&json))
    }

    #[test]
    fn the_default_profile_choice_round_trips_and_keeps_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{"theme": "nord"}"#).unwrap();

        assert_eq!(read_back(&path), None);
        write_default_rule_profile_pref_at(&path, Some("Company"));
        assert_eq!(read_back(&path).as_deref(), Some("Company"));
        write_default_rule_profile_pref_at(&path, Some("Other"));
        assert_eq!(read_back(&path).as_deref(), Some("Other"));

        write_default_rule_profile_pref_at(&path, None);
        assert_eq!(read_back(&path), None);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["theme"], "nord", "other settings survive every write");
        assert!(json.get("default_rule_profile").is_none());
    }

    #[test]
    fn a_blank_or_non_string_choice_reads_as_automatic() {
        for raw in [
            r#"{"default_rule_profile": ""}"#,
            r#"{"default_rule_profile": "   "}"#,
            r#"{"default_rule_profile": 3}"#,
            r#"{}"#,
        ] {
            let json: serde_json::Value = serde_json::from_str(raw).unwrap();
            assert_eq!(
                read_default_rule_profile_pref_from(Some(&json)),
                None,
                "{raw}"
            );
        }
        assert_eq!(read_default_rule_profile_pref_from(None), None);
    }

    #[test]
    fn a_broken_prefs_file_is_not_overwritten_by_the_choice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, b"{not json").unwrap();
        write_default_rule_profile_pref_at(&path, Some("Company"));
        assert_eq!(std::fs::read(&path).unwrap(), b"{not json");
    }
}
