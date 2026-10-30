//! #1275 — creating the child sheet of a sheet symbol, and the dialog a
//! double-click on a symbol without a file opens.

use std::path::{Path, PathBuf};

use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use super::super::dock::project_navigation::add_io;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Signex, TabInfo, TabKind};

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
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(top.clone());
    (app, top, id)
}

/// Open the dialog for the fixture's symbol and type `text` in its field.
fn dialog_with(app: &mut Signex, id: Uuid, text: &str) {
    app.open_create_child_sheet_dialog(id, false);
    let _ = app.handle_create_child_sheet_path_changed(text.to_owned());
}

fn confirm(app: &Signex) -> crate::app::contracts::CreateChildSheetConfirmState {
    app.ui_state
        .create_child_sheet_confirm
        .clone()
        .expect("the dialog is up")
}

fn file_field(app: &Signex, top: &Path) -> String {
    app.document_state.engines[top].document().child_sheets[0]
        .filename
        .clone()
}

#[test]
fn the_dialog_shows_the_default_path_beside_the_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    app.open_create_child_sheet_dialog(id, false);
    let c = confirm(&app);
    assert_eq!(c.path_text, "sheet1.snxsch");
    assert_eq!((c.parent.as_path(), c.symbol), (top.as_path(), id));
    assert_eq!(c.project_dir, dir.path());
    assert!(
        !dir.path().join("sheet1.snxsch").exists(),
        "nothing written yet"
    );
}

#[test]
fn a_sub_folder_path_creates_the_folder_and_file_registers_and_links_the_symbol() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    dialog_with(&mut app, id, "power/regulator.snxsch");
    let plan = app.plan_child_sheet_from_dialog().expect("a plan");
    let child = dir.path().join("power").join("regulator.snxsch");
    assert_eq!(plan.path, child);
    assert!(
        !dir.path().join("power").exists(),
        "planning writes nothing"
    );

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
            .any(|s| s.filename == "power/regulator.snxsch")
    );
    assert!(app.document_state.is_document_dirty(&project.path));
    assert_eq!(
        file_field(&app, &top),
        "power/regulator.snxsch",
        "the File field follows"
    );
}

#[test]
fn the_file_field_change_is_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    dialog_with(&mut app, id, "power/regulator.snxsch");
    let plan = app.plan_child_sheet_from_dialog().expect("a plan");
    add_io::write_new_schematic(&plan.path, plan.document).expect("written");
    let _ = app.handle_child_sheet_created(plan.project_id, &top, id, plan.path, Ok(()));
    assert_eq!(file_field(&app, &top), "power/regulator.snxsch");
    // The child opened in the active tab; undo the parent's own history.
    app.document_state
        .engines
        .get_mut(&top)
        .expect("parent engine")
        .undo()
        .expect("undo")
        .expect("one step");
    assert_eq!(file_field(&app, &top), "sheet1.snxsch");
}

#[test]
fn a_path_outside_the_project_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("proj");
    std::fs::create_dir(&project).expect("project dir");
    let (mut app, _top, id) = parent_with_symbol(&project);
    for text in [
        "../escape.snxsch",
        "/tmp/elsewhere.snxsch",
        "../../escape.snxsch",
    ] {
        dialog_with(&mut app, id, text);
        assert!(app.plan_child_sheet_from_dialog().is_none(), "{text}");
        let c = confirm(&app);
        assert!(c.error.is_some(), "{text}: an inline error");
        assert!(!c.exists);
    }
    assert!(!dir.path().join("escape.snxsch").exists());
    assert!(app.document_state.project_writes.is_empty());
    // A bad extension is refused too.
    dialog_with(&mut app, id, "x.txt");
    assert!(app.plan_child_sheet_from_dialog().is_none());
}

#[test]
fn an_existing_file_is_never_overwritten_and_can_be_opened_instead() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    let child = dir.path().join("sheet1.snxsch");
    std::fs::write(&child, "mine").expect("existing");
    dialog_with(&mut app, id, "sheet1.snxsch");
    assert!(
        app.plan_child_sheet_from_dialog().is_none(),
        "nothing to write"
    );
    let c = confirm(&app);
    assert!(c.exists && c.error.is_some(), "offers Open it");
    let err = add_io::write_new_schematic(&child, SchematicSheet::default()).unwrap_err();
    assert_eq!(err, add_io::ChildSheetWriteError::AlreadyExists);
    assert_eq!(std::fs::read_to_string(&child).expect("read"), "mine");
    // Open it: the dialog closes, the file is untouched, the symbol links to it.
    let _ = app.handle_create_child_sheet_open_existing();
    assert!(app.ui_state.create_child_sheet_confirm.is_none());
    assert_eq!(std::fs::read_to_string(&child).expect("read"), "mine");
    assert_eq!(file_field(&app, &top), "sheet1.snxsch");
}

#[test]
fn later_keeps_the_symbol_and_writes_nothing() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    app.open_create_child_sheet_dialog(id, false);
    let _ = app.handle_create_child_sheet_answered(false);
    assert!(app.ui_state.create_child_sheet_confirm.is_none());
    assert!(!dir.path().join("sheet1.snxsch").exists());
    assert_eq!(file_field(&app, &top), "sheet1.snxsch");
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("was left without a file"))
    );
}

#[test]
fn a_tab_switch_while_the_dialog_is_open_still_targets_the_original_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    dialog_with(&mut app, id, "kid.snxsch");
    switch_to_other_sheet_with_same_symbol(&mut app, dir.path(), id);
    let plan = app
        .plan_child_sheet_from_dialog()
        .expect("a plan for the original sheet");
    assert_eq!(plan.parent, top);
    assert_eq!(plan.path, dir.path().join("kid.snxsch"));
    assert!(!dir.path().join("sub").join("kid.snxsch").exists());
}

#[test]
fn a_symbol_that_is_gone_is_logged_not_ignored() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, id) = parent_with_symbol(dir.path());
    dialog_with(&mut app, id, "kid.snxsch");
    app.ui_state
        .create_child_sheet_confirm
        .as_mut()
        .expect("dialog")
        .symbol = Uuid::now_v7();
    assert!(app.plan_child_sheet_from_dialog().is_none());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("is no longer in"))
    );
}

#[test]
fn committing_the_place_sheet_tool_opens_the_dialog_with_the_default_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, _id) = parent_with_symbol(dir.path());
    let _ = app.place_sheet_click(10.0, 10.0);
    assert!(
        app.ui_state.create_child_sheet_confirm.is_none(),
        "first corner only"
    );
    let task = app.place_sheet_click(40.0, 40.0);
    crate::app::path_ops::settle_task(&mut app, task);
    let c = confirm(&app);
    assert_eq!(c.path_text, "sheet2.snxsch");
    assert_eq!(
        app.document_state.engines[&c.parent]
            .document()
            .child_sheets
            .len(),
        2
    );
}

#[test]
fn descending_through_a_symbol_without_a_file_opens_the_dialog() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, id) = parent_with_symbol(dir.path());
    app.descend_through_sheet_symbol(id, "sheet1.snxsch");
    assert_eq!(confirm(&app).symbol, id);
    assert!(
        app.has_locking_confirm_modal(),
        "a confirm locks the native menu"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Sheet 'sheet1' has no file yet"))
    );
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
}

#[test]
fn a_project_closed_during_the_write_is_logged() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    dialog_with(&mut app, id, "kid.snxsch");
    let plan = app.plan_child_sheet_from_dialog().expect("plan");
    add_io::write_new_schematic(&plan.path, plan.document).expect("written");
    app.document_state.projects.clear();
    let _ = app.handle_child_sheet_created(plan.project_id, &top, id, plan.path, Ok(()));
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("its project was closed first"))
    );
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
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(other.clone());
    other
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
fn a_project_being_renamed_refuses_inline_in_the_dialog_not_in_a_notice() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, id) = parent_with_symbol(dir.path());
    let project = app.document_state.projects[0].id;
    app.document_state.project_folder_renames.insert(project);
    dialog_with(&mut app, id, "kid.snxsch");
    assert!(app.plan_child_sheet_from_dialog().is_none());
    let c = confirm(&app);
    assert!(
        c.error.as_deref().is_some_and(|e| e.contains("renamed")),
        "{c:?}"
    );
    assert!(
        app.document_state.error_notice.is_none(),
        "no notice over the dialog"
    );
}

#[test]
fn open_it_on_a_file_another_symbol_uses_keeps_the_dialog_and_registers_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, id) = parent_with_symbol(dir.path());
    // A second symbol already points at taken.snxsch.
    let mut other = signex_engine::child_sheet_from_corners(
        Point::new(200.0, 50.0),
        Point::new(230.0, 80.0),
        "taken",
    )
    .expect("big enough");
    other.filename = "taken.snxsch".into();
    app.document_state
        .engines
        .get_mut(&top)
        .expect("engine")
        .execute(signex_engine::Command::PlaceChildSheet { child_sheet: other })
        .expect("placed");
    std::fs::write(dir.path().join("taken.snxsch"), "x").expect("existing file");
    dialog_with(&mut app, id, "taken.snxsch");
    assert!(app.plan_child_sheet_from_dialog().is_none());
    assert!(confirm(&app).exists);
    let listed_before = app.document_state.projects[0].data.sheets.len();
    let _ = app.handle_create_child_sheet_open_existing();
    let c = confirm(&app);
    assert!(
        c.error
            .as_deref()
            .is_some_and(|e| e.contains("already points")),
        "{c:?}"
    );
    assert_eq!(
        app.document_state.projects[0].data.sheets.len(),
        listed_before
    );
    assert_eq!(
        file_field(&app, &top),
        "sheet1.snxsch",
        "the field is unchanged"
    );
}

#[test]
fn the_place_tool_stays_armed_after_the_dialog_and_later() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, _id) = parent_with_symbol(dir.path());
    app.interaction_state.current_tool = crate::app::Tool::Sheet;
    let _ = app.place_sheet_click(10.0, 10.0);
    let task = app.place_sheet_click(40.0, 40.0);
    crate::app::path_ops::settle_task(&mut app, task);
    assert!(app.ui_state.create_child_sheet_confirm.is_some());
    let _ = app.handle_create_child_sheet_answered(false);
    assert!(
        app.ui_state.create_child_sheet_confirm.is_none(),
        "Later is immediate"
    );
    assert_eq!(app.interaction_state.current_tool, crate::app::Tool::Sheet);
}

/// The hint the lock layer shows over the main window.
const LOCK_HINT: &str = "A dialog is open: click to show it, Esc to cancel";

fn main_window_shows_the_lock(app: &Signex) -> bool {
    let main = app.ui_state.main_window_id.expect("main window");
    crate::layout_probe::lay_out_texts(app.view(main), iced::Size::new(1400.0, 900.0))
        .iter()
        .any(|t| t.text == LOCK_HINT)
}

#[test]
fn the_main_window_is_locked_while_create_sheet_is_open_by_either_route() {
    // Placement route.
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, _id) = parent_with_symbol(dir.path());
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    assert!(!main_window_shows_the_lock(&app), "no dialog yet");
    let _ = app.place_sheet_click(10.0, 10.0);
    let task = app.place_sheet_click(40.0, 40.0);
    crate::app::path_ops::settle_task(&mut app, task);
    let _ = app.sync_detached_modals();
    assert!(
        app.detached_window_of(crate::app::state::ModalId::CreateChildSheet)
            .is_some(),
        "the dialog has its window"
    );
    assert!(app.locking_confirm_window().is_some());
    assert!(
        main_window_shows_the_lock(&app),
        "placement route: the lock layer is drawn"
    );

    // Descend route.
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _top, id) = parent_with_symbol(dir.path());
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    app.descend_through_sheet_symbol(id, "sheet1.snxsch");
    let _ = app.sync_detached_modals();
    assert!(
        main_window_shows_the_lock(&app),
        "descend route: the lock layer is drawn"
    );
}

// ---- #859: the place tool's folder listing runs on a worker.

use crate::persist::run_task::outputs;

/// The second corner of a Place Sheet drag: nothing placed yet, the worker's
/// completion in hand.
fn commit_second_corner(app: &mut Signex) -> crate::app::Message {
    let _ = app.place_sheet_click(10.0, 10.0);
    let task = app.place_sheet_click(40.0, 40.0);
    let mut messages = outputs(task);
    assert_eq!(messages.len(), 1, "one completion per worker");
    messages.remove(0)
}

#[test]
fn the_second_corner_lists_the_folder_on_a_worker_not_in_update() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, _id) = parent_with_symbol(dir.path());
    let _ = app.place_sheet_click(10.0, 10.0);

    let task = app.place_sheet_click(40.0, 40.0);

    assert_eq!(
        app.document_state.engines[&top]
            .document()
            .child_sheets
            .len(),
        1,
        "nothing is placed before the listing arrives"
    );
    // A file that appears after `update` returned is still seen: the worker
    // lists the folder when it runs.
    std::fs::write(dir.path().join("sheet2.snxsch"), b"x").expect("file");
    crate::app::path_ops::settle_task(&mut app, task);
    assert_eq!(confirm(&app).path_text, "sheet3.snxsch");
}

#[test]
fn a_sheet_is_not_placed_when_the_active_sheet_changed_meanwhile() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, _id) = parent_with_symbol(dir.path());
    let message = commit_second_corner(&mut app);
    // The engine the corners were drawn on is replaced (a restore, a reopen).
    let sheet = app.document_state.engines[&top].document().clone();
    let replacement = signex_engine::Engine::new(sheet).expect("engine");
    app.document_state.engines.insert(top.clone(), replacement);
    let since = crate::diagnostics::newest_entry_id();

    let task = app.update(message);
    crate::app::path_ops::settle_task(&mut app, task);

    assert_eq!(
        app.document_state.engines[&top]
            .document()
            .child_sheets
            .len(),
        1
    );
    assert!(app.ui_state.create_child_sheet_confirm.is_none());
    assert!(crate::diagnostics::recent_entries().iter().any(|e| {
        Some(e.id) > since
            && e.message
                .contains("Sheet not placed: the sheet it was drawn on")
    }));
}

#[test]
fn a_second_commit_while_the_listing_runs_is_refused() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, _id) = parent_with_symbol(dir.path());
    let first = commit_second_corner(&mut app);
    let since = crate::diagnostics::newest_entry_id();
    let _ = app.place_sheet_click(60.0, 60.0);
    let second = app.place_sheet_click(90.0, 90.0);
    crate::app::path_ops::settle_task(&mut app, second);
    assert!(crate::diagnostics::recent_entries().iter().any(|e| {
        Some(e.id) > since
            && e.message
                .contains("the previous sheet is still being placed")
    }));

    let task = app.update(first);
    crate::app::path_ops::settle_task(&mut app, task);

    assert_eq!(
        app.document_state.engines[&top]
            .document()
            .child_sheets
            .len(),
        2,
        "only the first sheet was placed"
    );
}

#[test]
fn a_dead_listing_worker_places_nothing_and_says_so() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, _id) = parent_with_symbol(dir.path());
    let _unrun = commit_second_corner(&mut app);
    let ticket = 1;
    let since = crate::diagnostics::newest_entry_id();

    let task = app.update(crate::app::Message::Project(
        crate::app::ProjectMsg::SheetFilesListed {
            ticket,
            result: Err("the background task failed".to_string()),
        },
    ));
    crate::app::path_ops::settle_task(&mut app, task);

    assert_eq!(
        app.document_state.engines[&top]
            .document()
            .child_sheets
            .len(),
        1
    );
    assert!(crate::diagnostics::recent_entries().iter().any(|e| {
        Some(e.id) > since
            && e.message
                .contains("Sheet not placed: the folder could not be listed")
    }));
}
