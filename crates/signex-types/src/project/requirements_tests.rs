//! `requirements` in the `.snxprj` (#2339): absent by default, read entry by
//! entry, an entry this build cannot read kept as written, and a value that
//! is not a list kept for re-save.

use serde_json::json;

use super::*;
use crate::violation::Severity;

fn project_file(json: &str) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    std::fs::write(&path, json).unwrap();
    (path, dir)
}

fn file_with(requirements: serde_json::Value) -> String {
    json!({
        "name": "P", "dir": "", "schematic_root": null, "pcb_file": null,
        "requirements": requirements,
    })
    .to_string()
}

#[test]
fn a_project_without_requirements_saves_no_key() {
    let (path, _dir) =
        project_file(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#);
    let data = parse_project(&path).unwrap();
    assert!(data.requirements.is_empty());
    assert!(!data.has_unreadable_requirements());
    write_project(&path, &data).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("requirements"), "{saved}");
}

#[test]
fn every_kind_reads_with_its_parameters() {
    let (path, _dir) = project_file(&file_with(json!([
        {"id": "erc", "kind": "erc_clean"},
        {"id": "drc", "kind": "drc_clean", "allow_warnings": true, "severity": "warning"},
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80.5},
        {"id": "parts", "kind": "parts_state"},
        {"id": "usb", "kind": "net_class", "nets": "USB_*", "class": "USB", "min_trace_width_mm": 0.2},
        {"id": "cost", "kind": "bom_cost", "quantity": 100, "currency": "USD", "max_unit_cost": 12.5},
    ])));
    let data = parse_project(&path).unwrap();
    let known: Vec<&Requirement> = data
        .requirements
        .iter()
        .map(|entry| match entry {
            RequirementEntry::Known { requirement, .. } => requirement,
            RequirementEntry::Unreadable { reason, .. } => panic!("unreadable: {reason}"),
        })
        .collect();
    assert_eq!(known.len(), 6);
    assert_eq!(known[0].severity, Severity::Error);
    assert_eq!(
        known[1].kind,
        RequirementKind::DrcClean {
            allow_warnings: true
        }
    );
    assert_eq!(known[1].severity, Severity::Warning);
    assert_eq!(
        known[2].kind,
        RequirementKind::BoardSize {
            max_width_mm: 100.0,
            max_height_mm: 80.5
        }
    );
    assert_eq!(
        known[3].kind,
        RequirementKind::PartsState {
            states: vec!["released".to_string()]
        }
    );
    assert_eq!(known[4].kind.name(), "net_class");
    assert_eq!(
        known[5].kind,
        RequirementKind::BomCost {
            quantity: 100,
            currency: "USD".into(),
            max_unit_cost: 12.5
        }
    );
}

#[test]
fn an_unreadable_entry_is_kept_as_written_and_says_why() {
    let unknown_kind = json!({"id": "rails", "kind": "no_such_kind", "rail": "3V3"});
    let missing_field = json!({"id": "size", "kind": "board_size", "max_width_mm": 100});
    let extra_key = json!({"id": "erc", "kind": "erc_clean", "allow_warning": true});
    let (path, _dir) = project_file(&file_with(json!([
        unknown_kind.clone(),
        missing_field.clone(),
        extra_key.clone(),
    ])));
    let data = parse_project(&path).unwrap();
    let reasons: Vec<&str> = data
        .requirements
        .iter()
        .map(|entry| match entry {
            RequirementEntry::Unreadable { reason, .. } => reason.as_str(),
            RequirementEntry::Known { requirement, .. } => panic!("{} read", requirement.id),
        })
        .collect();
    assert!(reasons[0].contains("no_such_kind"), "{}", reasons[0]);
    assert!(reasons[1].contains("max_height_mm"), "{}", reasons[1]);
    assert_eq!(reasons[2], "unknown field `allow_warning`");
    assert_eq!(data.requirements[0].id(), Some("rails"));

    write_project(&path, &data).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        saved["requirements"],
        json!([unknown_kind, missing_field, extra_key])
    );
}

#[test]
fn a_requirements_value_that_is_not_a_list_is_kept_for_re_save() {
    let (path, _dir) = project_file(&file_with(json!({"id": "erc"})));
    let data = parse_project(&path).unwrap();
    assert!(data.requirements.is_empty());
    assert!(data.has_unreadable_requirements());
    write_project(&path, &data).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["requirements"], json!({"id": "erc"}));
}

#[test]
fn known_requirements_round_trip() {
    let list = json!([
        {"id": "erc", "kind": "erc_clean", "severity": "error"},
        {"id": "usb", "kind": "net_class", "nets": "USB_*", "severity": "warning",
         "min_clearance_mm": 0.15},
    ]);
    let (path, _dir) = project_file(&file_with(list.clone()));
    let data = parse_project(&path).unwrap();
    write_project(&path, &data).unwrap();
    let again = parse_project(&path).unwrap();
    assert_eq!(again.requirements, data.requirements);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["requirements"], list);
}

#[test]
fn a_hand_written_entry_saves_as_written() {
    // No severity, integer mm, no `states`, an explicit `null`: a save
    // writes none of the defaults and keeps the number forms.
    let list = json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
        {"id": "parts", "kind": "parts_state"},
        {"id": "usb", "kind": "net_class", "nets": "USB_*", "class": null},
    ]);
    let (path, _dir) = project_file(&file_with(list.clone()));
    let data = parse_project(&path).unwrap();
    assert!(data.requirements.iter().all(|e| e.requirement().is_some()));
    write_project(&path, &data).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        serde_json::to_string(&saved["requirements"]).unwrap(),
        serde_json::to_string(&list).unwrap()
    );
    assert!(!text.contains("100.0"), "{text}");
    assert!(!text.contains("severity"), "{text}");
    assert!(!text.contains("released"), "{text}");
}
