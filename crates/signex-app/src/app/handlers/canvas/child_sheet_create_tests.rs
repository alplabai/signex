//! #1275 — creating the child sheet of a sheet symbol, and the dialog a
//! double-click on a symbol without a file opens.

use std::path::{Path, PathBuf};

use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use super::super::dock::project_navigation::add_io;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, Signex, TabInfo, TabKind};

/// An app with `top.snxsch` open (and written) in a project, holding one sheet
/// symbol `sheet1` -> `sheet1.snxsch` with two entries.
fn parent_with_symbol(dir: &Path) -> (Signex, PathBuf, Uuid) {
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    let mut symbol = signex_engine::child_sheet_from_corners(
        Point::new(100.0, 50.0),
        Point::new(130.0, 80.0),
        "sheet1",
    )
    .expect("big enough");
    let clone = symbol.clone();
    symbol.pins.push(signex_engine::new_sheet_entry(
        &clone,
        Point::new(98.0, 60.0),
        "CLK",
        Some("input"),
    ));
    symbol.pins.push(signex_engine::new_sheet_entry(
        &clone,
        Point::new(133.0, 60.0),
        "DATA",
        Some("output"),
    ));
    let id = symbol.uuid;
    let sheet = SchematicSheet {
        child_sheets: vec![symbol],
        ..SchematicSheet::default()
    };
    std::fs::write(&top, "parent").expect("parent on disk");
    let engine = signex_engine::Engine::new_with_path(sheet, Some(top.clone())).expect("engine");
    app.document_state.engines.insert(top.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "top".into(),
        path: top.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(top.clone());
    (app, top, id)
}

#[test]
fn the_child_is_written_beside_the_parent_with_one_port_per_entry_and_registered() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    let plan = app
        .plan_child_sheet_creation(&top, id, None)
        .expect("a plan");
    let child = dir.path().join("sheet1.snxsch");
    assert_eq!(plan.path, child);
    assert!(!child.exists(), "planning writes nothing");

    add_io::write_new_schematic(&plan.path, plan.document).expect("written");
    let text = std::fs::read_to_string(&child).expect("file");
    let loaded = signex_types::format::SnxSchematic::parse(&text).expect("parses");
    let ports = signex_engine::exposed_sheet_ports(&loaded.sheet);
    let got: Vec<(&str, &str)> = ports
        .iter()
        .map(|p| (p.name.as_str(), p.direction.as_str()))
        .collect();
    assert_eq!(got, [("CLK", "input"), ("DATA", "output")]);

    let _ = app.handle_child_sheet_created(plan.project_id, &top, id, child.clone(), Ok(()));
    let project = &app.document_state.projects[0];
    assert!(
        project
            .data
            .sheets
            .iter()
            .any(|s| s.filename == "sheet1.snxsch")
    );
    assert!(
        app.document_state.dirty_paths.contains(&project.path),
        "the project is marked changed"
    );
}

#[test]
fn an_existing_child_file_is_never_overwritten() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    let child = dir.path().join("sheet1.snxsch");
    std::fs::write(&child, "mine").expect("existing");
    assert!(
        app.plan_child_sheet_creation(&top, id, None).is_none(),
        "nothing to write"
    );
    let err = add_io::write_new_schematic(&child, SchematicSheet::default()).unwrap_err();
    assert_eq!(err, add_io::ChildSheetWriteError::AlreadyExists);
    assert_eq!(std::fs::read_to_string(&child).expect("read"), "mine");
}

#[test]
fn descending_through_a_symbol_without_a_file_asks_instead_of_doing_nothing() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, id) = parent_with_symbol(dir.path());
    app.descend_through_sheet_symbol(id, "sheet1.snxsch");
    let confirm = app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("the dialog is up");
    assert_eq!(confirm.symbol, id);
    assert_eq!(confirm.path, dir.path().join("sheet1.snxsch"));
    assert!(
        app.has_locking_confirm_modal(),
        "a confirm dialog locks the native menu"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Sheet 'sheet1' has no file yet"))
    );
    // Declining writes nothing.
    let _ = app.update(Message::Project(ProjectMsg::CreateChildSheetAnswered {
        create: false,
    }));
    assert!(app.ui_state.create_child_sheet_confirm.is_none());
    assert!(!dir.path().join("sheet1.snxsch").exists());
}

#[test]
fn an_unsaved_parent_says_so_and_offers_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    std::fs::remove_file(&top).expect("parent not on disk");
    app.descend_through_sheet_symbol(id, "sheet1.snxsch");
    assert!(app.ui_state.create_child_sheet_confirm.is_none());
    let card = app.document_state.error_notice.as_ref().expect("a notice");
    assert!(
        card.detail.contains("Save the parent sheet first"),
        "{card:?}"
    );
    assert!(app.plan_child_sheet_creation(&top, id, None).is_none());
}

#[test]
fn the_sheet_entry_tool_places_an_entry_on_the_edge_and_undo_removes_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, id) = parent_with_symbol(dir.path());
    let pins = |app: &Signex| {
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .child_sheets[0]
            .pins
            .clone()
    };
    assert_eq!(pins(&app).len(), 2);

    // Off every symbol: nothing is placed.
    app.place_sheet_entry_click(10.0, 10.0);
    assert_eq!(pins(&app).len(), 2);

    // Beside the symbol's left edge: snaps onto it, default name and direction.
    app.place_sheet_entry_click(98.0, 70.0);
    let placed = pins(&app);
    assert_eq!(placed.len(), 3);
    let entry = &placed[2];
    assert_eq!((entry.position.x, entry.rotation), (100.0, 0.0));
    assert_eq!(
        (entry.name.as_str(), entry.direction.as_str()),
        ("P1", "input")
    );
    assert!(!entry.auto_generated);

    app.document_state
        .active_engine_mut()
        .expect("engine")
        .undo()
        .expect("undo")
        .expect("one step");
    assert_eq!(pins(&app).len(), 2);
    let _ = id;
}

/// A second sheet that carries a copy of the same symbol, made active.
fn switch_to_other_sheet_with_same_symbol(app: &mut Signex, dir: &Path, id: Uuid) -> PathBuf {
    let top = app.document_state.tabs[0].path.clone();
    let symbol = app.document_state.engines[&top].document().child_sheets[0].clone();
    assert_eq!(symbol.uuid, id);
    let other = dir.join("sub").join("other.snxsch");
    std::fs::create_dir_all(other.parent().expect("dir")).expect("mkdir");
    std::fs::write(&other, "other").expect("other on disk");
    let sheet = SchematicSheet {
        child_sheets: vec![symbol],
        ..SchematicSheet::default()
    };
    let engine = signex_engine::Engine::new_with_path(sheet, Some(other.clone())).expect("engine");
    app.document_state.engines.insert(other.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "other".into(),
        path: other.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(other.clone());
    other
}

#[test]
fn the_answer_plans_against_the_sheet_that_was_asked_not_the_active_tab() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    app.descend_through_sheet_symbol(id, "sheet1.snxsch");
    let shown = app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("dialog")
        .path
        .clone();
    switch_to_other_sheet_with_same_symbol(&mut app, dir.path(), id);

    let confirm = app
        .ui_state
        .create_child_sheet_confirm
        .clone()
        .expect("dialog");
    assert_eq!(confirm.parent, top);
    let plan = app
        .plan_child_sheet_creation(&confirm.parent, id, Some(&confirm.path))
        .expect("a plan for the original sheet");
    assert_eq!(plan.path, shown, "written where the dialog said");
    assert!(!dir.path().join("sub").join("sheet1.snxsch").exists());
}

#[test]
fn a_path_other_than_the_one_shown_is_refused_with_a_log_line() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    let wrong = dir.path().join("elsewhere.snxsch");
    assert!(
        app.plan_child_sheet_creation(&top, id, Some(&wrong))
            .is_none()
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("not the file you were asked about"))
    );
    assert!(app.document_state.project_writes.is_empty());
    assert!(!dir.path().join("sheet1.snxsch").exists());
}

#[test]
fn a_symbol_that_is_gone_is_logged_not_ignored() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, _id) = parent_with_symbol(dir.path());
    assert!(
        app.plan_child_sheet_creation(&top, Uuid::new_v4(), None)
            .is_none()
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("is no longer in"))
    );
}

#[test]
fn a_case_variant_of_a_listed_sheet_is_not_listed_twice() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, _id) = parent_with_symbol(dir.path());
    let project = &mut app.document_state.projects[0];
    project.case_fold = crate::app::state::scope::CaseFold::FOLD;
    project.data.sheets.clear();
    project.data.schematic_root = None;
    assert!(super::super::dock::project_navigation::register_sheet_entry(project, "Sheet1.snxsch"));
    assert_eq!(
        project.data.schematic_root.as_deref(),
        Some("Sheet1.snxsch")
    );
    assert!(
        !super::super::dock::project_navigation::register_sheet_entry(project, "sheet1.snxsch")
    );
    assert_eq!(project.data.sheets.len(), 1);
}

#[test]
fn a_project_closed_during_the_write_is_logged() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    let plan = app.plan_child_sheet_creation(&top, id, None).expect("plan");
    add_io::write_new_schematic(&plan.path, plan.document).expect("written");
    app.document_state.projects.clear();
    let _ = app.handle_child_sheet_created(plan.project_id, &top, id, plan.path, Ok(()));
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("its project was closed first"))
    );
}
