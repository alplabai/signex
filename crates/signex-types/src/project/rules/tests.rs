use crate::project::{ProjectData, parse_project, write_project};

const WITH_RULES: &str = r#"{
  "name": "p",
  "dir": "",
  "schematic_root": null,
  "pcb_file": null,
  "rules": {
    "schema": 1,
    "profile": {"name": "Signex Default", "source": "builtin", "version": "1.0", "sha256": "abc"},
    "erc": {
      "severity": {"pin_type_conflict": "error", "future_rule": "warning"},
      "pin_matrix": {"output/output": "error"},
      "future_erc_key": [1, 2]
    },
    "drc": {"clearance_nm": 150000}
  }
}"#;

#[test]
fn rules_round_trip_with_unknown_ids_and_sections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.snxprj");
    std::fs::write(&path, WITH_RULES).unwrap();
    let data = parse_project(&path).unwrap();
    let rules = data.rules.as_ref().expect("rules section");
    assert_eq!(rules.erc.severity["pin_type_conflict"], "error");
    assert_eq!(rules.erc.severity["future_rule"], "warning");
    assert_eq!(rules.profile.as_ref().unwrap().name, "Signex Default");

    write_project(&path, &data).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["rules"]["drc"]["clearance_nm"], 150000);
    assert_eq!(json["rules"]["erc"]["future_erc_key"][1], 2);
    assert_eq!(json["rules"]["erc"]["severity"]["future_rule"], "warning");
    assert_eq!(parse_project(&path).unwrap().rules, data.rules);
}

#[test]
fn project_without_rules_resaves_without_a_rules_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.snxprj");
    let data: ProjectData =
        serde_json::from_str(r#"{"name":"p","dir":"","schematic_root":null,"pcb_file":null}"#)
            .unwrap();
    assert!(data.rules.is_none());
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&first).contains("rules"));
    // `dir` is re-derived on load, so compare two load -> save cycles: the
    // file is byte-stable and never grows a `rules` key.
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    let second = std::fs::read(&path).unwrap();
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), second);
    assert!(!String::from_utf8_lossy(&second).contains("rules"));
}

#[test]
fn rules_serialize_in_deterministic_key_order() {
    let mut rules = crate::project::ProjectRules::default();
    rules.erc.severity.insert("z_rule".into(), "off".into());
    rules.erc.severity.insert("a_rule".into(), "error".into());
    let text = serde_json::to_string(&rules).unwrap();
    assert!(text.find("a_rule").unwrap() < text.find("z_rule").unwrap());
}

// ---------------------------------------------------------------------------
// Forward compatibility (#1180): a section this build cannot interpret never
// stops the project from opening, is not applied, and re-saves untouched.
// ---------------------------------------------------------------------------

fn project_with_rules(rules: &str) -> String {
    format!(r#"{{"name":"p","dir":"","schematic_root":null,"pcb_file":null,"rules":{rules}}}"#)
}

/// Open `text`, save it, and return the parsed project and the saved JSON.
fn open_and_resave(text: &str) -> (ProjectData, serde_json::Value) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.snxprj");
    std::fs::write(&path, text).unwrap();
    let data = parse_project(&path).expect("the project opens");
    write_project(&path, &data).unwrap();
    let saved = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    (data, saved)
}

fn rules_of(text: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(text).unwrap()["rules"].clone()
}

#[test]
fn a_changed_value_shape_is_kept_not_applied() {
    let text =
        project_with_rules(r#"{"schema":2,"erc":{"severity":{"unused_pin":{"level":"error"}}}}"#);
    let (data, saved) = open_and_resave(&text);
    assert!(data.rules.is_none());
    assert!(data.has_unreadable_rules());
    assert_eq!(saved["rules"], rules_of(&text));
}

#[test]
fn a_shape_change_without_a_schema_bump_is_kept_not_applied() {
    let text = project_with_rules(r#"{"erc":{"severity":{"unused_pin":["error"]}}}"#);
    let (data, saved) = open_and_resave(&text);
    assert!(data.has_unreadable_rules());
    assert_eq!(saved["rules"], rules_of(&text));
}

#[test]
fn a_newer_schema_is_kept_not_applied_even_when_its_values_parse() {
    let text = project_with_rules(r#"{"schema":2,"erc":{"severity":{"unused_pin":"error"}}}"#);
    let (data, saved) = open_and_resave(&text);
    assert!(data.rules.is_none(), "v2 meaning must not be read as v1");
    assert_eq!(saved["rules"], rules_of(&text));
}

#[test]
fn a_profile_without_a_name_opens_and_round_trips() {
    let text = project_with_rules(r#"{"schema":1,"profile":{"source":"user"}}"#);
    let (data, saved) = open_and_resave(&text);
    let profile = data.rules.as_ref().unwrap().profile.as_ref().unwrap();
    assert_eq!(profile.source, "user");
    assert_eq!(saved["rules"], rules_of(&text));
}

#[test]
fn a_section_this_build_does_not_own_resaves_unchanged() {
    let text = project_with_rules(r#"{"drc":{"x":1}}"#);
    let (data, saved) = open_and_resave(&text);
    assert!(data.rules.is_some());
    assert_eq!(saved["rules"], rules_of(&text), "no schema or erc added");
}

#[test]
fn a_rule_set_stored_over_a_kept_section_replaces_it() {
    let text = project_with_rules(r#"{"schema":2,"erc":{"severity":{"x":{"level":"off"}}}}"#);
    let (mut data, _) = open_and_resave(&text);
    let mut stored = crate::project::ProjectRules::default();
    stored
        .erc
        .severity
        .insert("unused_pin".into(), "error".into());
    data.rules = Some(stored.clone());

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.snxprj");
    write_project(&path, &data).unwrap();
    let back = parse_project(&path).unwrap();
    assert_eq!(back.rules, Some(stored));
    assert!(!back.has_unreadable_rules());
}
