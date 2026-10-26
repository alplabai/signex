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
