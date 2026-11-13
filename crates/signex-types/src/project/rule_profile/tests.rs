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
fn the_drc_section_is_read_and_round_trips_with_unknown_keys() {
    let profile = RuleProfile::from_json_bytes(
        br#"{"name":"C","version":"1","drc":{"severity":{"builtin::unconnected":"error"},"dsl":"rule a","future":[1]}}"#,
    )
    .unwrap();
    assert_eq!(profile.drc.severity["builtin::unconnected"], "error");
    assert_eq!(profile.drc.dsl.as_deref(), Some("rule a"));
    let json: serde_json::Value = serde_json::from_slice(&profile.to_json_bytes()).unwrap();
    assert_eq!(json["drc"]["future"][0], 1);
    assert_eq!(
        RuleProfile::from_json_bytes(&profile.to_json_bytes()).unwrap(),
        profile
    );
}

#[test]
fn a_profile_without_drc_resaves_without_it() {
    let text = b"{\n  \"schema\": 1,\n  \"name\": \"C\",\n  \"version\": \"1\"\n}\n";
    let profile = RuleProfile::from_json_bytes(text).unwrap();
    assert!(profile.drc.is_empty());
    assert_eq!(profile.to_json_bytes(), text);
}

/// A `drc` of another shape is refused with the whole profile, as any other
/// shape change is, rather than half-read.
#[test]
fn a_profile_with_a_malformed_drc_section_is_refused() {
    for drc in [r#""legacy""#, "null", "[1]", r#"{"dsl":5}"#] {
        let text =
            format!(r#"{{"name":"C","erc":{{"severity":{{"unused_pin":"error"}}}},"drc":{drc}}}"#);
        let err = RuleProfile::from_json_bytes(text.as_bytes()).unwrap_err();
        assert!(matches!(err, RuleProfileError::Parse(_)), "{drc}");
    }
}

/// A profile without a `drc` section keeps the hash it had before DRC rules
/// entered the hash (#2307 step 2b): the ERC hash, so no recorded
/// provenance or drift result moves.
#[test]
fn a_profile_without_drc_hashes_as_its_erc_section() {
    let mut a = RuleProfile::from_json_bytes(SAMPLE.as_bytes()).unwrap();
    a.drc = DrcRuleSet::default();
    assert_eq!(a.content_sha256(), a.erc.sha256_hex());
    assert_eq!(
        crate::project::rule_set_sha256(&a.erc, &a.drc),
        a.erc.sha256_hex()
    );
}

/// Every part of a `drc` section enters the content hash (#2307 step 2b,
/// decision 6824136804): severities, the rule text and foreign keys.
#[test]
fn the_drc_section_enters_the_profile_content_hash() {
    let a = RuleProfile::from_json_bytes(SAMPLE.as_bytes()).unwrap();
    assert_ne!(a.content_sha256(), a.erc.sha256_hex());
    let edits: [fn(&mut DrcRuleSet); 3] = [
        |d| {
            d.severity.insert("builtin::short".into(), "off".into());
        },
        |d| d.dsl = Some("rule x".into()),
        |d| d.unknown = serde_json::from_str(r#"{"clearance_nm": 1}"#).unwrap(),
    ];
    for edit in edits {
        let mut b = a.clone();
        edit(&mut b.drc);
        assert_ne!(a.content_sha256(), b.content_sha256(), "{:?}", b.drc);
    }
    let mut only_drc = a.clone();
    only_drc.erc = ErcRules::default();
    assert_ne!(only_drc.content_sha256(), a.content_sha256());
    assert_ne!(only_drc.content_sha256(), ErcRules::default().sha256_hex());
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
