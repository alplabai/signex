//! Editing the requirements list (#2339): each edit keeps the untouched
//! fields as written, refuses what the check could not read, and saves and
//! reads back the same list.

use serde_json::{Value, json};

use super::*;
use crate::project::{RequirementKind, parse_project, write_project};

fn project(list: Value) -> ProjectData {
    let mut data: ProjectData = serde_json::from_value(
        json!({"name": "P", "dir": "", "schematic_root": null, "pcb_file": null}),
    )
    .expect("a project");
    data.requirements = serde_json::from_value(list).expect("a list");
    data
}

fn raw(data: &ProjectData, index: usize) -> &Value {
    data.requirements[index].raw()
}

#[test]
fn a_new_requirement_starts_from_its_kind_and_takes_a_free_id() {
    let mut data = project(json!([{"id": "board_size", "kind": "erc_clean"}]));
    assert_eq!(data.add_requirement("board_size", None), Ok(1));
    assert_eq!(
        raw(&data, 1),
        &json!({"id": "board_size_2", "kind": "board_size",
                "max_width_mm": 100, "max_height_mm": 100})
    );
    for kind in REQUIREMENT_KINDS {
        let index = data.add_requirement(kind, None).expect(kind);
        let entry = &data.requirements[index];
        let requirement = entry.requirement().expect("readable");
        assert_eq!(requirement.kind.name(), kind);
        assert!(requirement.enabled);
    }
    assert_eq!(
        data.add_requirement("rail_budget", None),
        Err(RequirementProblem::UnknownKind("rail_budget".into()))
    );
    assert_eq!(
        data.add_requirement("erc_clean", Some("board_size")),
        Err(RequirementProblem::IdTaken("board_size".into()))
    );
}

#[test]
fn a_field_is_set_from_typed_text_and_the_rest_stays_as_written() {
    let mut data = project(json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
    ]));
    data.set_requirement_field(0, RequirementField::MaxHeightMm, " 75.5 ")
        .unwrap();
    assert_eq!(
        raw(&data, 0),
        &json!({"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 75.5})
    );
    data.set_requirement_field(0, RequirementField::Severity, "Warning")
        .unwrap();
    data.set_requirement_field(0, RequirementField::Id, "outline")
        .unwrap();
    let requirement = data.requirements[0].requirement().unwrap();
    assert_eq!(requirement.id, "outline");
    assert_eq!(requirement.severity, crate::violation::Severity::Warning);
    assert_eq!(
        requirement.kind,
        RequirementKind::BoardSize {
            max_width_mm: 100.0,
            max_height_mm: 75.5
        }
    );
}

#[test]
fn a_refused_edit_changes_nothing_and_says_why() {
    let list = json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
        {"id": "usb", "kind": "net_class", "nets": "USB_*", "class": "USB"},
        {"id": "cost", "kind": "bom_cost", "quantity": 10, "currency": "USD", "max_unit_cost": 5},
        {"id": "parts", "kind": "parts_state"},
        {"id": "rails", "kind": "rail_budget"},
    ]);
    let mut data = project(list.clone());
    let field = RequirementField::MaxWidthMm;
    for typed in ["", "0", "-3", "wide", "inf", "NaN"] {
        assert!(
            data.set_requirement_field(0, field, typed).is_err(),
            "{typed}"
        );
    }
    assert_eq!(
        data.set_requirement_field(0, RequirementField::Id, "usb"),
        Err(RequirementProblem::IdTaken("usb".into()))
    );
    assert_eq!(
        data.set_requirement_field(0, RequirementField::Id, "  "),
        Err(RequirementProblem::IdEmpty)
    );
    assert_eq!(
        data.set_requirement_field(0, RequirementField::Nets, "*"),
        Err(RequirementProblem::NotAField {
            kind: "board_size".into(),
            field: "nets".into()
        })
    );
    assert_eq!(
        data.set_requirement_field(1, RequirementField::Class, ""),
        Err(RequirementProblem::NetClassAsksNothing)
    );
    assert_eq!(
        data.set_requirement_field(2, RequirementField::Quantity, "2.5"),
        Err(RequirementProblem::NotACount("2.5".into()))
    );
    assert_eq!(
        data.set_requirement_field(3, RequirementField::States, "released, shipped"),
        Err(RequirementProblem::UnknownState("shipped".into()))
    );
    assert_eq!(
        data.set_requirement_field(4, RequirementField::Id, "rail"),
        Err(RequirementProblem::EntryUnreadable("rails".into()))
    );
    assert_eq!(
        data.set_requirement_enabled(9, false),
        Err(RequirementProblem::NoEntry(9))
    );
    assert_eq!(data.requirements, project(list).requirements);
}

#[test]
fn turning_off_writes_the_flag_and_turning_on_leaves_it_out() {
    let mut data = project(json!([{"id": "erc", "kind": "erc_clean"}]));
    data.set_requirement_enabled(0, false).unwrap();
    assert_eq!(
        raw(&data, 0),
        &json!({"id": "erc", "kind": "erc_clean", "enabled": false})
    );
    assert!(!data.requirements[0].requirement().unwrap().enabled);
    data.set_requirement_enabled(0, true).unwrap();
    assert_eq!(raw(&data, 0), &json!({"id": "erc", "kind": "erc_clean"}));
}

#[test]
fn an_unreadable_entry_can_be_removed_and_an_unreadable_list_is_not_edited() {
    let mut data = project(json!([{"id": "rails", "kind": "rail_budget"}]));
    data.remove_requirement(0).unwrap();
    assert!(data.requirements.is_empty());
    data.unknown
        .insert(super::super::REQUIREMENTS_KEY.into(), json!({"id": "x"}));
    assert_eq!(
        data.add_requirement("erc_clean", None),
        Err(RequirementProblem::ListUnreadable)
    );
}

#[test]
fn an_id_names_one_entry() {
    let data = project(json!([
        {"id": "a", "kind": "erc_clean"},
        {"id": "b", "kind": "erc_clean"},
        {"id": "b", "kind": "drc_clean"},
    ]));
    assert_eq!(data.requirement_index(" a "), Ok(0));
    assert_eq!(
        data.requirement_index("b"),
        Err(RequirementProblem::AmbiguousId("b".into()))
    );
    assert_eq!(
        data.requirement_index("c"),
        Err(RequirementProblem::UnknownId("c".into()))
    );
}

#[test]
fn an_edited_list_saves_and_reads_back_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    let mut data = project(json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
    ]));
    data.add_requirement("parts_state", None).unwrap();
    data.set_requirement_field(1, RequirementField::States, "released,in_review")
        .unwrap();
    data.set_requirement_enabled(0, false).unwrap();
    write_project(&path, &data).unwrap();
    let again = parse_project(&path).unwrap();
    assert_eq!(again.requirements, data.requirements);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("100.0"), "{text}");
    assert_eq!(
        again.requirements[1].requirement().unwrap().kind,
        RequirementKind::PartsState {
            states: vec!["released".into(), "in_review".into()]
        }
    );
}
