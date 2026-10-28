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
    let plan = app.plan_child_sheet_creation(id).expect("a plan");
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
    let (mut app, _top, id) = parent_with_symbol(dir.path());
    let child = dir.path().join("sheet1.snxsch");
    std::fs::write(&child, "mine").expect("existing");
    assert!(
        app.plan_child_sheet_creation(id).is_none(),
        "nothing to write"
    );
    let err = add_io::write_new_schematic(&child, SchematicSheet::default()).unwrap_err();
    assert!(add_io::is_already_exists(&err), "{err}");
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
    assert!(app.plan_child_sheet_creation(id).is_none());
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
