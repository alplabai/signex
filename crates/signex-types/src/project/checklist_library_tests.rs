//! The design checklist past its first edits (#1291, mockup row 206): an
//! item's history, the user's saved templates and the template a checklist
//! names, Reset from template, Import from another project, the CSV export,
//! design-object targets and the one-item run's report.

use serde_json::json;

use super::*;
use crate::check::{CheckReport, Finding, Severity};
use crate::schematic::{Label, LabelType, Point, SchematicSheet, SelectedKind, Symbol};

fn empty_project() -> ProjectData {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    std::fs::write(
        &path,
        r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#,
    )
    .unwrap();
    parse_project(&path).unwrap()
}

fn stamp(day: &str) -> EditStamp {
    EditStamp::new(Some(day), Some("j.berg"))
}

fn item(stage: ChecklistStage, title: &str, link: Option<ChecklistLink>) -> NewChecklistItem {
    NewChecklistItem {
        stage,
        title: title.into(),
        link,
    }
}

fn started() -> ProjectData {
    let mut data = empty_project();
    let items = vec![
        item(
            ChecklistStage::Schematic,
            "ERC clean",
            Some(ChecklistLink::Erc),
        ),
        item(ChecklistStage::Layout, "Hot loop, short", None),
    ];
    data.start_design_checklist(Some("prototype_build"), items, &stamp("2026-11-18"))
        .unwrap();
    data
}

fn events(data: &ProjectData, id: &str) -> Vec<(String, Option<String>)> {
    let checklist = data.design_checklist().unwrap().unwrap();
    checklist
        .item(id)
        .unwrap()
        .history
        .iter()
        .map(|line| (line.event.clone(), line.value.clone()))
        .collect()
}

#[test]
fn every_change_is_a_history_line_and_a_change_of_nothing_writes_none() {
    let mut data = started();
    assert_eq!(
        events(&data, "1"),
        [("created".to_string(), Some("prototype_build".to_string()))]
    );
    let first = &data.design_checklist().unwrap().unwrap().items[0];
    assert_eq!(first.history[0].by.as_deref(), Some("j.berg"));
    assert_eq!(first.history[0].on.as_deref(), Some("2026-11-18"));
    assert_eq!(first.updated.as_deref(), Some("2026-11-18"));
    data.set_checklist_field("1", ChecklistField::Owner, "j.berg", &stamp("2026-11-19"))
        .unwrap();
    data.set_checklist_field("1", ChecklistField::Owner, "j.berg", &stamp("2026-11-20"))
        .unwrap();
    data.set_checklist_field(
        "1",
        ChecklistField::Link,
        "requirement:REQ-2",
        &stamp("2026-11-25"),
    )
    .unwrap();
    data.set_checklist_state(
        "1",
        ItemState::Waived,
        Some("Debug only"),
        &stamp("2026-11-25"),
    )
    .unwrap();
    assert_eq!(
        events(&data, "1"),
        [
            ("created".to_string(), Some("prototype_build".to_string())),
            ("owner".to_string(), Some("j.berg".to_string())),
            ("link".to_string(), Some("requirement:REQ-2".to_string())),
            ("waived".to_string(), Some("Debug only".to_string())),
        ],
        "setting the owner it already has writes no line"
    );
    let line = &data.design_checklist().unwrap().unwrap().items[0].history[1];
    assert_eq!(line.event(), Some(HistoryEvent::Owner));
}

#[test]
fn the_history_keeps_its_newest_lines_and_an_unknown_event_reads() {
    let mut data = started();
    for n in 0..HISTORY_LIMIT + 5 {
        let title = format!("Title {n}");
        data.set_checklist_field("2", ChecklistField::Title, &title, &stamp("2026-11-20"))
            .unwrap();
    }
    let checklist = data.design_checklist().unwrap().unwrap();
    let history = &checklist.item("2").unwrap().history;
    assert_eq!(history.len(), HISTORY_LIMIT);
    let last = format!("Title {}", HISTORY_LIMIT + 4);
    assert_eq!(
        history.last().unwrap().value.as_deref(),
        Some(last.as_str())
    );

    let read: HistoryEntry =
        serde_json::from_value(json!({"event": "signed_off", "by": "m.demir"})).unwrap();
    assert_eq!(read.event(), None);
    assert_eq!(serde_json::to_value(&read).unwrap()["event"], "signed_off");
}

#[test]
fn a_template_ref_reads_back_and_a_saved_template_starts_the_same_items() {
    for text in ["release_review", "blank", "user:House review 2026"] {
        assert_eq!(TemplateRef::parse(text).unwrap().key(), text);
    }
    assert_eq!(TemplateRef::parse("user: "), None);
    assert_eq!(TemplateRef::parse("audit"), None);

    let data = started();
    let checklist = data.design_checklist().unwrap().unwrap();
    assert_eq!(
        checklist.to_saved_template("  ", None),
        Err(ChecklistProblem::EmptyName)
    );
    let saved = checklist
        .to_saved_template(" House review ", Some("buck-3v3"))
        .unwrap();
    assert_eq!(saved.name, "House review");
    assert_eq!(saved.key(), "user:House review");
    assert_eq!(saved.count(ChecklistStage::Layout), 1);
    let mut again = empty_project();
    again
        .start_design_checklist(
            Some(saved.key().as_str()),
            saved.new_items(),
            &stamp("2026-11-26"),
        )
        .unwrap();
    let copy = again.design_checklist().unwrap().unwrap();
    assert_eq!(copy.template.as_deref(), Some("user:House review"));
    assert_eq!(copy.items[0].link, Some(ChecklistLink::Erc));
    assert_eq!(copy.items[1].title, "Hot loop, short");
}

#[test]
fn the_library_file_keeps_unknown_keys_replaces_by_name_and_refuses_a_bad_file() {
    // The file's contents as the host reads them (the host test in
    // signex-app's checklist `library_file` writes them to disk).
    let path = std::path::Path::new("config").join(TEMPLATE_LIBRARY_FILE);
    assert_eq!(
        TemplateLibrary::from_file(&path, None),
        Ok(TemplateLibrary::default())
    );
    let template = |name: &str, title: &str| SavedTemplate {
        name: name.into(),
        saved_from: None,
        items: vec![SavedTemplateItem {
            stage: ChecklistStage::Fabrication,
            title: title.into(),
            link: None,
            unknown: UnknownJson::default(),
        }],
        unknown: UnknownJson::default(),
    };
    let (_, bytes) = TemplateLibrary::with_template(&path, None, template("A", "one")).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["synced_at"] = json!("2026-11-01");
    let file = serde_json::to_vec(&value).unwrap();
    let (library, bytes) =
        TemplateLibrary::with_template(&path, Some(file.as_slice()), template("A", "two")).unwrap();
    assert_eq!(library.templates.len(), 1, "same name: replaced");
    assert_eq!(library.find(" A ").unwrap().items[0].title, "two");
    let (library, bytes) =
        TemplateLibrary::with_template(&path, Some(bytes.as_slice()), template("B", "three"))
            .unwrap();
    assert_eq!(library.templates.len(), 2);
    let written: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(written["synced_at"], "2026-11-01");

    assert!(
        TemplateLibrary::with_template(&path, Some(&b"[not a library"[..]), template("C", "four"))
            .is_err()
    );
}

#[test]
fn a_reset_puts_the_template_back_and_keeps_the_checklist_keys() {
    let mut data = started();
    data.set_checklist_state("1", ItemState::Done, None, &stamp("2026-11-20"))
        .unwrap();
    data.add_checklist_item("layout", "Extra", &stamp("2026-11-20"))
        .unwrap();
    let mut stored = data.design_checklist_value().cloned().unwrap();
    stored["review_round"] = json!(2);
    data.restore_design_checklist_value(Some(stored));
    let items = vec![item(ChecklistStage::Schematic, "Fresh", None)];
    data.reset_design_checklist("blank", items, &stamp("2026-11-26"))
        .unwrap();
    let checklist = data.design_checklist().unwrap().unwrap();
    assert_eq!(checklist.template.as_deref(), Some("blank"));
    assert_eq!(checklist.items.len(), 1);
    assert_eq!(checklist.items[0].id, "1");
    assert_eq!(checklist.items[0].state, ItemState::Open);
    assert_eq!(data.design_checklist_value().unwrap()["review_round"], 2);

    let mut empty = empty_project();
    assert_eq!(
        empty.reset_design_checklist("blank", Vec::new(), &stamp("2026-11-26")),
        Err(ChecklistProblem::NotStarted)
    );
}

#[test]
fn an_import_brings_the_items_in_open_and_says_where_from() {
    let mut data = empty_project();
    let items = vec![item(ChecklistStage::Layout, "Hot loop", None)];
    data.import_design_checklist(
        "buck-3v3",
        Some("release_review"),
        items.clone(),
        &stamp("2026-11-26"),
    )
    .unwrap();
    let checklist = data.design_checklist().unwrap().unwrap();
    assert_eq!(checklist.template.as_deref(), Some("release_review"));
    assert_eq!(
        events(&data, "1"),
        [("imported".to_string(), Some("buck-3v3".to_string()))]
    );
    assert_eq!(
        data.import_design_checklist("other", None, items, &stamp("2026-11-26")),
        Err(ChecklistProblem::AlreadyStarted)
    );
}

#[test]
fn the_csv_has_a_header_of_file_keys_and_quotes_what_needs_it() {
    let mut data = started();
    data.set_checklist_state(
        "2",
        ItemState::Waived,
        Some("No \"room\""),
        &stamp("2026-11-21"),
    )
    .unwrap();
    let csv = data.design_checklist().unwrap().unwrap().to_csv();
    let lines: Vec<&str> = csv.split("\r\n").collect();
    assert_eq!(lines[0], CSV_COLUMNS.join(","));
    assert_eq!(lines[1], "1,schematic,open,ERC clean,,erc,,,2026-11-18");
    assert_eq!(
        lines[2],
        "2,layout,waived,\"Hot loop, short\",,,\"No \"\"room\"\"\",,2026-11-21"
    );
    assert_eq!(lines[3], "", "every row ends in CRLF");
}

fn sheet() -> SchematicSheet {
    let symbol = |reference: &str| Symbol {
        uuid: uuid::Uuid::now_v7(),
        reference: reference.into(),
        ..Default::default()
    };
    SchematicSheet {
        symbols: vec![symbol("U3"), symbol("C14"), symbol("#PWR01"), symbol("R?")],
        labels: vec![Label::new("VIN", Point::new(1.0, 2.0), LabelType::Net)],
        ..Default::default()
    }
}

#[test]
fn an_object_target_reads_back_and_finds_its_item_on_a_sheet() {
    for text in [
        "part:U3",
        "pin:U3.2",
        "net:VIN",
        "sheet:power.snxsch",
        "board",
    ] {
        assert_eq!(ObjectTarget::parse(text).to_text(), text);
    }
    assert_eq!(
        ObjectTarget::parse("U3 pin 2 (EN)"),
        ObjectTarget::Text("U3 pin 2 (EN)".into())
    );
    let sheet = sheet();
    let u3 = sheet.symbols[0].uuid;
    for text in ["part:U3", "pin:U3.2", "U3 pin 2 (EN)", "U3"] {
        let found = ObjectTarget::parse(text).find_on(&sheet).unwrap();
        assert_eq!(
            (found.uuid, found.kind),
            (u3, SelectedKind::Symbol),
            "{text}"
        );
    }
    let net = ObjectTarget::parse("net:VIN").find_on(&sheet).unwrap();
    assert_eq!(net.kind, SelectedKind::Label);
    assert_eq!(ObjectTarget::parse("VIN").find_on(&sheet), Some(net));
    assert_eq!(ObjectTarget::parse("part:U9").find_on(&sheet), None);
    assert_eq!(ObjectTarget::parse("sheet:a.snxsch").find_on(&sheet), None);

    let targets = sheet_targets(&sheet);
    assert_eq!(targets.parts, ["C14", "U3"]);
    assert_eq!(targets.nets, ["VIN"]);
}

fn failing(rule: &str) -> Finding {
    Finding {
        rule: rule.into(),
        severity: Severity::Error,
        message: String::new(),
        evidence: json!({}),
        items: Vec::new(),
        sheet: String::new(),
        key: rule.into(),
    }
}

#[test]
fn a_run_of_one_link_keeps_what_the_last_run_said_of_the_others() {
    let report = |findings: Vec<Finding>, passed: Vec<String>| {
        CheckReport::new("requirements", findings, Vec::new(), Vec::new(), Vec::new())
            .with_passed(passed)
    };
    let previous = report(
        vec![failing("req::checklist::erc")],
        vec!["req::REQ-1".into()],
    );
    let ran = report(Vec::new(), vec!["req::checklist::erc".into()]);
    let merged = merge_link_run(Some(&previous), ran, &["checklist::erc".into()]);
    let data = empty_project();
    assert_eq!(
        ChecklistLink::Erc.result(&data, Some(&merged)),
        LinkResult::Passes
    );
    let req = ChecklistLink::Requirement { id: "REQ-1".into() };
    assert_eq!(req.result(&data, Some(&merged)), LinkResult::Passes);
}

#[test]
fn mine_is_the_items_the_named_person_owns() {
    let mut data = started();
    data.set_checklist_field("2", ChecklistField::Owner, "J.Berg ", &stamp("2026-11-20"))
        .unwrap();
    let checklist = data.design_checklist().unwrap().unwrap();
    assert_eq!(checklist.owned_by("j.berg"), 1);
    assert_eq!(checklist.owned_by(" "), 0);
    assert!(checklist.item("2").unwrap().is_owned_by("j.berg"));
}
