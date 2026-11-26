//! #1291 (mockup row 206): an item's link. The link picker lists the checks,
//! the project's requirements and the parts, nets and sheets of its open
//! documents, and a picked row is the item's link; the link card's Open
//! shows a requirement in the Project tab and cross-probes a design object
//! to its sheet; Run check runs one item's check and keeps the others'
//! results. GUI path through `Signex::update` and the laid-out view, the
//! headless path through the catalog.

use serde_json::json;
use signex_types::project::{ChecklistTemplate, RequirementEntry};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Symbol};

use super::library_tests::{app_as_jberg, click, shows};
use super::tests::{answer, items, refusal, run, send, settle, tab};
use super::{ChecklistMsg, ChecklistNotice};
use crate::app::command::CommandOutcome;
use crate::app::state::ProjectId;
use crate::app::{ProjectSection, Signex, TabInfo, TabKind};

/// U3's uuid on the project's sheet.
const U3: uuid::Uuid = uuid::Uuid::from_u128(0x0003);

/// The project's sheet `a.snxsch`, open in a tab of its own, with U3 and C14
/// and a VIN label; the project lists requirement REQ-002; the checklist is
/// started from Prototype build.
fn linked_app() -> (Signex, ProjectId, std::path::PathBuf, tempfile::TempDir) {
    let (mut app, id, dir) = app_as_jberg();
    let symbol = |n: u128, reference: &str| Symbol {
        uuid: uuid::Uuid::from_u128(n),
        reference: reference.into(),
        position: Point::new(10.0, 10.0),
        ..Default::default()
    };
    let sheet = SchematicSheet {
        symbols: vec![symbol(0x0003, "U3"), symbol(0x0014, "C14")],
        labels: vec![Label::new("VIN", Point::new(1.0, 2.0), LabelType::Net)],
        ..Default::default()
    };
    let path = app.document_state.projects[0].dir().join("a.snxsch");
    crate::app::handlers::menu::export::tests::open_with(&mut app.document_state, &path, sheet);
    app.document_state.tabs.push(TabInfo {
        title: "a".into(),
        path: path.clone(),
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.projects[0]
        .data
        .requirements
        .push(RequirementEntry::read(
            json!({"id": "REQ-002", "kind": "erc_clean"}),
        ));
    super::tests::start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    (app, id, path, dir)
}

#[test]
fn the_picker_lists_checks_requirements_and_design_objects_and_a_pick_links() {
    let (mut app, id, _path, _dir) = linked_app();
    let choices = answer(&mut app, "project_checklist_link_choices", &[]);
    let listed = choices["choices"].as_array().cloned().unwrap_or_default();
    for (group, link) in [
        ("check", "erc"),
        ("check", "drc"),
        ("check", "title_block"),
        ("requirement", "requirement:REQ-002"),
        ("part", "object:part:C14"),
        ("part", "object:part:U3"),
        ("net", "object:net:VIN"),
        ("sheet", "object:sheet:a.snxsch"),
    ] {
        assert!(
            listed.contains(&json!({"group": group, "link": link})),
            "{group} {link}: {choices}"
        );
    }

    let _ = send(&mut app, id, ChecklistMsg::ItemSelected("4".into()));
    let _ = click(&mut app, id, "Link a design object or check…");
    assert_eq!(tab(&app, id).link_picker.as_deref(), Some("4"));
    assert!(shows(&app, id, "Parts"));
    let _ = send(&mut app, id, ChecklistMsg::LinkQueryEdited("u3".into()));
    let _ = click(&mut app, id, "Part U3");
    assert_eq!(
        items(&app)[3]["link"],
        json!({"kind": "object", "target": "part:U3"})
    );
    assert_eq!(tab(&app, id).link_picker, None, "a pick closes the picker");
    assert_eq!(items(&app)[3]["history"][1]["event"], "link");

    // Typed text that names nothing listed links as a design object.
    let _ = send(
        &mut app,
        id,
        ChecklistMsg::LinkPickerToggled { item: "4".into() },
    );
    let _ = send(
        &mut app,
        id,
        ChecklistMsg::LinkQueryEdited("U7 pin 3".into()),
    );
    let _ = click(&mut app, id, "Link to object:U7 pin 3");
    assert_eq!(items(&app)[3]["link"]["target"], "U7 pin 3");
    let _ = click(&mut app, id, "Remove link");
    assert!(items(&app)[3].get("link").is_none());
}

#[test]
fn open_in_requirements_shows_the_requirement_in_the_project_tab() {
    let (mut app, id, _path, _dir) = linked_app();
    let link = ChecklistMsg::LinkPicked {
        item: "5".into(),
        link: "requirement:REQ-002".into(),
    };
    let _ = send(&mut app, id, link);
    let _ = send(&mut app, id, ChecklistMsg::ItemSelected("5".into()));
    assert!(shows(&app, id, "Requirement REQ-002"));
    let task = click(&mut app, id, "Open in Requirements");
    settle(&mut app, task);
    let docs = &app.document_state;
    assert_eq!(docs.tabs[docs.active_tab].kind.as_project(), Some(id));
    assert_eq!(docs.project_tabs[&id].section, ProjectSection::Requirements);

    let value = answer(&mut app, "project_open_checklist_link", &["item=5"]);
    assert_eq!(value["opens"], "requirements");
    let reason = refusal(&mut app, "project_open_checklist_link", &["item=6"]);
    assert!(reason.contains("links to nothing"), "{reason}");
}

#[test]
fn a_design_object_link_cross_probes_to_its_sheet() {
    let (mut app, id, path, _dir) = linked_app();
    let link = ChecklistMsg::LinkPicked {
        item: "4".into(),
        link: "object:part:U3".into(),
    };
    let _ = send(&mut app, id, link);
    // A click on the row's link words cross-probes.
    let task = click(&mut app, id, "Part U3");
    settle(&mut app, task);
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(path.as_path())
    );

    let value = answer(&mut app, "project_open_checklist_link", &["item=4"]);
    assert_eq!(value["opens"], "sheet");
    assert_eq!(value["document"], path.display().to_string());
    assert_eq!(value["object"], U3.to_string());

    let missing = ChecklistMsg::LinkPicked {
        item: "4".into(),
        link: "object:part:U99".into(),
    };
    let _ = send(&mut app, id, missing);
    let value = answer(&mut app, "project_open_checklist_link", &["item=4"]);
    assert_eq!(value["opens"], "not_found");
    assert_eq!(
        tab(&app, id).notice,
        Some(ChecklistNotice::LinkNotFound("part:U99".into()))
    );
    assert!(shows(
        &app,
        id,
        "part:U99 is not in the project's open documents."
    ));
}

#[test]
fn run_check_runs_one_items_check_and_keeps_the_others_results() {
    let (mut app, id, _path, _dir) = linked_app();
    // Item 3 links the DRC run; the project lists no board, so it fails.
    let _ = send(&mut app, id, ChecklistMsg::ItemSelected("3".into()));
    let task = click(&mut app, id, "Run check");
    settle(&mut app, task);
    let read = answer(&mut app, "project_checklist", &[]);
    assert_eq!(read["items"][2]["result"], "fails");
    assert_eq!(
        read["items"][0]["result"], "not_run",
        "item 1's ERC did not run"
    );

    let task = send(&mut app, id, ChecklistMsg::RunPressed);
    settle(&mut app, task);
    let all = answer(&mut app, "project_checklist", &[]);
    assert!(matches!(
        run(&mut app, "project_run_checklist", &["item=3"]),
        CommandOutcome::Ran
    ));
    let again = answer(&mut app, "project_checklist", &[]);
    assert_eq!(again["items"][0]["result"], all["items"][0]["result"]);
    assert_eq!(again["items"][2]["result"], "fails");
    let reason = refusal(&mut app, "project_run_checklist", &["item=99"]);
    assert!(reason.contains("no item `99`"), "{reason}");
}
