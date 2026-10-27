use super::*;

const SAMPLE: &str = r#"{
  "schema": 1,
  "name": "Company",
  "version": "2.1",
  "erc": {
    "severity": {"pin_type_conflict": "error", "future_rule": "warning"},
    "pin_matrix": {"output/output": "error"},
    "future_erc_key": [1, null]
  },
  "drc": {"clearance_nm": 150000}
}"#;

#[test]
fn profile_round_trips_and_keeps_unknown_keys() {
    let profile = RuleProfile::from_json_bytes(SAMPLE.as_bytes()).unwrap();
    assert_eq!(profile.name, "Company");
    assert_eq!(profile.erc.severity["future_rule"], "warning");

    let again = RuleProfile::from_json_bytes(&profile.to_json_bytes()).unwrap();
    assert_eq!(again, profile);
    let json: serde_json::Value = serde_json::from_slice(&profile.to_json_bytes()).unwrap();
    assert_eq!(json["drc"]["clearance_nm"], 150000);
    assert_eq!(json["erc"]["future_erc_key"][1], serde_json::Value::Null);
    assert_eq!(json["erc"]["severity"]["future_rule"], "warning");
}

#[test]
fn saved_bytes_are_stable_and_ordered() {
    let profile = RuleProfile::from_json_bytes(SAMPLE.as_bytes()).unwrap();
    let first = profile.to_json_bytes();
    let second = RuleProfile::from_json_bytes(&first)
        .unwrap()
        .to_json_bytes();
    assert_eq!(first, second);
    let text = String::from_utf8(first).unwrap();
    assert!(text.ends_with("}\n"));
    assert!(text.find("future_rule").unwrap() < text.find("pin_type_conflict").unwrap());
}

#[test]
fn a_newer_schema_is_refused() {
    let err = RuleProfile::from_json_bytes(br#"{"schema": 2, "name": "x"}"#).unwrap_err();
    assert!(matches!(err, RuleProfileError::UnsupportedSchema(2)));
    assert!(RuleProfile::from_json_bytes(b"[1]").is_err());
}

#[test]
fn hash_follows_the_rules_not_the_labels() {
    let a = RuleProfile::from_json_bytes(SAMPLE.as_bytes()).unwrap();
    let mut renamed = a.clone();
    renamed.name = "Other".into();
    renamed.version = "9".into();
    assert_eq!(a.content_sha256(), renamed.content_sha256());
    let mut changed = a.clone();
    changed
        .erc
        .severity
        .insert("pin_type_conflict".into(), "off".into());
    assert_ne!(a.content_sha256(), changed.content_sha256());
    assert_eq!(a.content_sha256().len(), 64);
}
