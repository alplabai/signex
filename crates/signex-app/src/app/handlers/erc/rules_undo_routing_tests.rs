//! Undo / Redo of a project's stored ERC rules follow the window the stroke
//! came from (#1209): the ERC dialog steps its rule history and never falls
//! through to the sheet behind it, and the main window keeps sheet undo while
//! the dialog is open. Driven through `dispatch_command`, the keyboard's
//! entry point, not through the undo helpers.

use signex_erc::{RuleKind, Severity};
use signex_types::schematic::{SelectedItem, SelectedKind, Symbol};

use crate::app::command::CommandArgs;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::{ModalId, WindowKind};
use crate::app::{Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;

/// A project over a fresh directory whose root sheet is open with one
/// symbol, so the sheet has an undo history to protect.
fn app_with_sheet() -> (Signex, uuid::Uuid) {
    let dir = std::env::temp_dir().join(format!("signex-erc-route-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    // `Signex::new` loads the machine preferences, which other tests write;
    // Store seeds from them, and an edit that matches the seed is a no-op.
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let path = dir.join("a_root.snxsch");
    let symbol = uuid::Uuid::now_v7();
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: symbol,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    crate::app::handlers::menu::export::tests::open_with(&mut app.document_state, &path, sheet);
    app.document_state.tabs.push(TabInfo {
        title: "a_root.snxsch".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    (app, symbol)
}

fn move_symbol(app: &mut Signex, symbol: uuid::Uuid) {
    let moved = app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(symbol, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
            keep_wires: false,
        },
        true,
        true,
    );
    assert!(moved, "fixture: the move must change the sheet");
}

fn symbol_x(app: &Signex) -> f64 {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .position
        .x
}

fn rules(app: &Signex) -> Option<signex_types::project::ProjectRules> {
    app.document_state.projects[0].data.rules.clone()
}

/// Open the ERC dialog in its own OS window and return that window.
fn detach_erc(app: &mut Signex) -> iced::window::Id {
    app.ui_state.erc_dialog_open = true;
    let id = iced::window::Id::unique();
    app.ui_state
        .windows
        .insert(id, WindowKind::DetachedModal(ModalId::ErcDialog));
    id
}

fn press(app: &mut Signex, command: &str, window: Option<iced::window::Id>) {
    let id = AppCommandId::new(command).expect("catalog id");
    let _ = app.dispatch_command(&id, CommandArgs::none(), window);
}

/// Stored rules, one severity edit, then a sheet edit made afterwards.
fn stored_edited_then_moved() -> (Signex, uuid::Uuid) {
    let (mut app, symbol) = app_with_sheet();
    let _ = app.handle_store_erc_rules_in_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    move_symbol(&mut app, symbol);
    (app, symbol)
}

#[test]
fn undo_in_the_erc_window_steps_the_rules_and_leaves_the_sheet() {
    let (mut app, _) = stored_edited_then_moved();
    let moved_x = symbol_x(&app);
    let window = detach_erc(&mut app);

    press(&mut app, "undo", Some(window));

    let after = rules(&app).expect("the store is still in place");
    assert_ne!(after.erc.severity["unused_pin"], "info");
    assert_eq!(symbol_x(&app), moved_x, "the sheet move stays");
}

#[test]
fn undo_in_the_erc_window_never_falls_through_to_the_sheet() {
    let (mut app, _) = stored_edited_then_moved();
    let moved_x = symbol_x(&app);
    let window = detach_erc(&mut app);

    // Edit, then Store: two steps. A third and fourth press find the rule
    // history empty.
    for _ in 0..4 {
        press(&mut app, "undo", Some(window));
    }

    assert!(rules(&app).is_none(), "both rule steps were undone");
    assert_eq!(symbol_x(&app), moved_x, "an empty rule history ends there");
}

#[test]
fn redo_in_the_erc_window_replays_the_rules_not_the_sheet() {
    let (mut app, _) = stored_edited_then_moved();
    let edited = rules(&app);
    let window = detach_erc(&mut app);
    press(&mut app, "undo", Some(window));
    assert_ne!(rules(&app), edited);

    press(&mut app, "redo", Some(window));

    assert_eq!(rules(&app), edited);
}

#[test]
fn undo_in_the_main_window_undoes_the_sheet_while_the_erc_window_is_open() {
    let (mut app, _) = stored_edited_then_moved();
    let start = symbol_x(&app) - 10.0;
    let edited = rules(&app);
    let _erc = detach_erc(&mut app);
    let main = app.ui_state.main_window_id;

    press(&mut app, "undo", main);

    assert_eq!(symbol_x(&app), start, "the main window undoes its sheet");
    assert_eq!(rules(&app), edited, "the hidden rule edit is untouched");
}

#[test]
fn undo_in_the_main_window_with_no_sheet_history_leaves_the_rules_alone() {
    let (mut app, _) = app_with_sheet();
    let _ = app.handle_store_erc_rules_in_project();
    let stored = rules(&app);
    let _erc = detach_erc(&mut app);
    let main = app.ui_state.main_window_id;

    press(&mut app, "undo", main);

    assert_eq!(rules(&app), stored, "Ctrl+Z on the canvas cannot unstore");
}

#[test]
fn undo_and_redo_are_enabled_by_the_rule_history_only_in_the_erc_window() {
    let (mut app, _) = app_with_sheet();
    let _ = app.handle_store_erc_rules_in_project();
    let window = detach_erc(&mut app);
    let main = app.ui_state.main_window_id;

    let erc = app.command_context(Some(window));
    assert!(erc.can_undo && !erc.can_redo);
    let canvas = app.command_context(main);
    assert!(!canvas.can_undo, "the sheet has no history of its own");

    press(&mut app, "undo", Some(window));
    let erc = app.command_context(Some(window));
    assert!(!erc.can_undo && erc.can_redo);
}

#[test]
fn clicking_the_selected_severity_records_nothing_and_stays_clean() {
    let (mut app, _) = app_with_sheet();
    let _ = app.handle_store_erc_rules_in_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    let path = app.document_state.projects[0].path.clone();
    app.document_state.mark_documents_clean_where(|_| true);
    let steps = |app: &Signex| {
        let history = &app.document_state.rules_history[&app.document_state.projects[0].id];
        (history.can_undo(), history.can_redo())
    };
    let before = steps(&app);

    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);

    assert!(!app.document_state.is_document_dirty(&path));
    assert_eq!(steps(&app), before);
    // One undo returns to the seeded set: the no-op added no step of its own.
    assert!(app.undo_project_rules());
    assert_ne!(rules(&app).unwrap().erc.severity["unused_pin"], "info");
    assert!(app.undo_project_rules());
    assert!(!app.undo_project_rules());
}

#[test]
fn a_new_edit_after_an_undo_drops_the_redo_branch() {
    let (mut app, _) = app_with_sheet();
    let _ = app.handle_store_erc_rules_in_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    assert!(app.undo_project_rules());
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    let edited = rules(&app);

    assert!(!app.redo_project_rules());
    assert_eq!(rules(&app), edited);
}

/// A project whose `rules` section a newer Signex wrote keeps it; Store must
/// not seed a set from this machine over it.
#[test]
fn store_is_refused_when_the_project_holds_an_unreadable_rule_set() {
    let (mut app, _) = app_with_sheet();
    let dir = tempfile::Builder::new()
        .prefix("signex-erc-newer-")
        .tempdir()
        .expect("temp dir");
    let file = dir.path().join("newer.snxprj");
    std::fs::write(
        &file,
        r#"{"name":"p","dir":"","schematic_root":null,"pcb_file":null,
            "rules":{"schema":99,"erc":{"severity":{"dangling_wire":{"level":"off"}}}}}"#,
    )
    .unwrap();
    let parsed = signex_types::project::parse_project(&file).expect("the project opens");
    assert!(parsed.has_unreadable_rules());
    let project = &mut app.document_state.projects[0].data;
    project.rules = parsed.rules;
    project.unknown = parsed.unknown;

    let _ = app.handle_store_erc_rules_in_project();

    assert!(rules(&app).is_none(), "the newer set is not replaced");
    assert!(app.document_state.projects[0].data.has_unreadable_rules());
}
