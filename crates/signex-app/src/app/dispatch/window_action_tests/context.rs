//! #1966 round 2: Net Classes, Edit Symbol and command enablement in an
//! undocked window.

use std::path::PathBuf;

use super::*;
use crate::app::ProjectMsg;
use crate::app::handlers::document_files::restore_commit_tests::collect_messages;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};
use crate::panels::{NetClassField, NetClassMsg, PanelMsg};
use signex_types::net_class::NetClass;

/// Run `task` and feed every message back through `update`, following the
/// tasks those return.
fn run_task(app: &mut Signex, task: iced::Task<Message>) {
    for message in collect_messages(task) {
        let next = app.update(message);
        run_task(app, next);
    }
}

// ---- Net Classes ----

/// Two classes in the project the main window has active, "Signal" selected
/// in main's panel; the undocked window shows another sheet.
fn net_class_fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap().keep();
    let app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    let mut f = fixture_in_dirs(app, &dir, &dir);
    let project = &mut f.app.document_state.projects[0];
    project.data.net_classes = vec![NetClass::named("Power"), NetClass::named("Signal")];
    f.app.document_state.active_project = Some(f.app.document_state.projects[0].id);
    f.app.refresh_panel_ctx();
    let _ = f
        .app
        .update(dock(PanelMsg::NetClass(NetClassMsg::Select(1))));
    f
}

fn net_class_message(f: &Fixture, message: NetClassMsg) -> Message {
    f.app
        .scope_dock_message(f.window, dock(PanelMsg::NetClass(message)))
}

#[test]
fn a_net_class_name_typed_in_a_window_shows_in_that_window() {
    let mut f = net_class_fixture();
    let typed = net_class_message(&f, NetClassMsg::Draft(NetClassField::Name, "POWER".into()));
    let _ = f.app.update(typed);
    let window = &f.app.document_state.window_panel_ctx[&f.window].net_classes;
    assert_eq!(window.draft.name, "POWER");
    assert_eq!(window.selected, 1);
}

#[test]
fn a_net_class_commit_from_a_window_edits_the_class_the_window_shows() {
    let mut f = net_class_fixture();
    let shown = f.app.document_state.window_panel_ctx[&f.window]
        .net_classes
        .selected;
    for message in [
        NetClassMsg::Draft(NetClassField::Name, "Rail".into()),
        NetClassMsg::Commit(NetClassField::Name),
    ] {
        let message = net_class_message(&f, message);
        let _ = f.app.update(message);
    }
    let names: Vec<_> = f.app.document_state.projects[0]
        .data
        .net_classes
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(names[shown], "Rail", "{names:?}");
    let window = &f.app.document_state.window_panel_ctx[&f.window].net_classes;
    assert_eq!(window.classes[shown].name, "Rail");
    assert_eq!(window.draft.name, "Rail");
}

// ---- Edit Symbol ----

const LIBRARY_ID: u128 = 0x1966_0001;
const SYMBOL_ID: u128 = 0x1966_0002;

/// The window's sheet holds one selected placed part whose symbol lives in an
/// open library under `dir`; returns the symbol file Edit Symbol resolves.
fn edit_symbol_fixture(dir: &std::path::Path) -> (Fixture, PathBuf) {
    let mut f = fixture();
    let library = uuid::Uuid::from_u128(LIBRARY_ID);
    let symbol = uuid::Uuid::from_u128(SYMBOL_ID);
    f.app
        .library
        .open_libraries
        .push(crate::library::state::OpenLibrary {
            root: dir.join("L.snxlib"),
            display_name: String::new(),
            library_id: library,
            tables: Default::default(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
        });
    let part = uuid::Uuid::from_u128(77);
    f.app
        .document_state
        .engines
        .get_mut(&f.window_path)
        .unwrap()
        .execute(signex_engine::Command::PlaceSymbol {
            symbol: signex_types::schematic::Symbol {
                uuid: part,
                lib_id: format!("{library}:{symbol}"),
                reference: "R1".into(),
                ..Default::default()
            },
        })
        .unwrap();
    let slot = f.app.interaction_state.canvases.get_mut(&f.window).unwrap();
    slot.selected = vec![SelectedItem::new(part, SelectedKind::Symbol)];
    let path = dir.join("symbols").join(format!("{symbol}.snxsym"));
    (f, path)
}

fn edit_symbol_in_window(f: &mut Fixture) -> iced::Task<Message> {
    f.app.run_command_in_window(
        f.window,
        Message::Library(LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(
            PlacedPartAction::EditSymbol,
        ))),
    )
}

fn open_symbol_tab(f: &mut Fixture, path: &std::path::Path) {
    f.app.document_state.tabs.push(TabInfo {
        title: "r".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.to_path_buf()),
    });
}

#[test]
fn opening_an_already_open_symbol_tab_from_a_window_swap_is_refused_there() {
    let _ = crate::diagnostics::init_logging();
    let (mut f, path) = edit_symbol_fixture(std::path::Path::new("/lib-1966"));
    open_symbol_tab(&mut f, &path);
    let _ = f
        .app
        .run_in_window_document(f.window, |app| app.handle_open_primitive(path.clone()));
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
    assert_eq!(f.app.document_state.active_tab, 0);
    assert_eq!(wires_shown_in(&f.app, f.window), Some(1));
    assert!(said(
        "Opening a symbol or footprint was not done from inside"
    ));
}

#[test]
fn edit_symbol_from_a_window_shows_an_already_open_symbol_tab_on_the_main_window() {
    let (mut f, path) = edit_symbol_fixture(std::path::Path::new("/lib-1966"));
    open_symbol_tab(&mut f, &path);
    let task = edit_symbol_in_window(&mut f);
    let queued = collect_messages(task);
    assert!(
        queued.iter().any(|m| matches!(
            m,
            Message::Project(ProjectMsg::PrimitiveOpenRequested(p)) if *p == path
        )),
        "{queued:?}"
    );
    assert_eq!(
        f.app.document_state.active_tab, 0,
        "nothing ran in the swap"
    );
    feed(&mut f.app, queued);
    assert_eq!(f.app.active_tab_path(), Some(path));
    assert_eq!(wires_shown_in(&f.app, f.window), Some(1));
}

#[test]
fn edit_symbol_from_a_window_opens_a_symbol_that_is_not_open_yet_on_the_main_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut f, path) = edit_symbol_fixture(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"))
        .to_toml_string()
        .unwrap();
    std::fs::write(&path, text).unwrap();
    let task = edit_symbol_in_window(&mut f);
    assert!(f.app.document_state.tabs.iter().all(|t| t.path != path));
    run_task(&mut f.app, task);
    assert!(
        f.app
            .document_state
            .tabs
            .iter()
            .any(|t| matches!(&t.kind, TabKind::SymbolEditor(p) if *p == path)),
        "the symbol editor tab opened"
    );
    assert_eq!(f.app.active_tab_path(), Some(path));
    assert_eq!(wires_shown_in(&f.app, f.window), Some(1));
}

// ---- Command enablement ----

#[test]
fn a_window_command_is_enabled_by_the_sheet_the_window_shows_not_mains_tab() {
    let mut f = fixture();
    // The main window has no schematic active (a PCB or library tab).
    f.app.document_state.active_path = None;
    assert!(!f.app.command_context(None).has_schematic);
    assert!(f.app.command_context(Some(f.window)).has_schematic);
}
