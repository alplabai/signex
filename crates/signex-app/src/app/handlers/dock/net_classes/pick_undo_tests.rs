//! #1220: Ctrl+Z after a net-class pick reverts the pick, in order with the
//! sheet's own edits.

use signex_types::net_class::NetClass;
use signex_types::schematic::{SelectedItem, SelectedKind, Symbol};

use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{Signex, TabInfo, TabKind};
use crate::panels::NetClassMsg;

fn app_with_sheet() -> (Signex, uuid::Uuid) {
    let dir = std::env::temp_dir().join(format!("signex-pick-undo-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    let path = dir.join("a_root.snxsch");
    let symbol = uuid::Uuid::new_v4();
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: symbol,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    open_with(&mut app.document_state, &path, sheet);
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
        .unwrap()
        .document()
        .symbols[0]
        .position
        .x
}

fn pick(app: &mut Signex, net: &str, class: Option<&str>) {
    let project = app.document_state.projects[0].id;
    app.handle_net_class_message(&NetClassMsg::AssignNet {
        project,
        net: net.to_string(),
        class: class.map(str::to_string),
    });
}

fn nets(app: &Signex) -> Vec<String> {
    app.document_state.projects[0].data.net_classes[0]
        .nets
        .clone()
}

#[test]
fn undo_reverts_the_pick_and_leaves_the_sheet_alone() {
    let (mut app, symbol) = app_with_sheet();
    move_symbol(&mut app, symbol);
    let moved_x = symbol_x(&app);
    pick(&mut app, "GND", Some("Power"));
    assert_eq!(nets(&app), ["GND"]);

    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the pick is reverted");
    assert_eq!(symbol_x(&app), moved_x, "the earlier sheet edit stays");
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );
}

#[test]
fn edit_pick_undo_twice_then_redo_twice_keeps_order() {
    let (mut app, symbol) = app_with_sheet();
    let start = symbol_x(&app);
    move_symbol(&mut app, symbol);
    let moved_x = symbol_x(&app);
    pick(&mut app, "GND", Some("Power"));

    app.handle_undo_requested();
    assert!(nets(&app).is_empty());
    assert_eq!(symbol_x(&app), moved_x);
    app.handle_undo_requested();
    assert_eq!(symbol_x(&app), start, "second undo reverts the sheet edit");

    app.handle_redo_requested();
    assert_eq!(symbol_x(&app), moved_x);
    assert!(nets(&app).is_empty(), "the sheet edit comes back first");
    app.handle_redo_requested();
    assert_eq!(nets(&app), ["GND"], "then the pick");
}

#[test]
fn each_pick_is_one_step_and_a_sheet_edit_after_a_pick_undoes_first() {
    let (mut app, symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    pick(&mut app, "VCC", Some("Power"));
    move_symbol(&mut app, symbol);
    let start = symbol_x(&app) - 10.0;

    app.handle_undo_requested();
    assert_eq!(symbol_x(&app), start);
    assert_eq!(nets(&app), ["GND", "VCC"]);
    app.handle_undo_requested();
    assert_eq!(nets(&app), ["GND"]);
    app.handle_undo_requested();
    assert!(nets(&app).is_empty());
}

#[test]
fn a_sheet_edit_ends_the_pick_redo_branch() {
    let (mut app, symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    app.handle_undo_requested();
    move_symbol(&mut app, symbol);
    app.handle_redo_requested();
    assert!(nets(&app).is_empty(), "the undone pick is not replayed");
}

fn panel(app: &mut Signex, msg: NetClassMsg) {
    app.handle_net_class_message(&msg);
}

fn class_count(app: &Signex) -> usize {
    app.document_state.projects[0].data.net_classes.len()
}

/// Open a second sheet next to the first and make it the active one.
fn open_second_sheet(app: &mut Signex) -> std::path::PathBuf {
    let path = app.document_state.projects[0]
        .path
        .with_file_name("b_sheet.snxsch");
    open_with(
        &mut app.document_state,
        &path,
        signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            paper_size: "A4".to_string(),
            ..Default::default()
        },
    );
    app.document_state.tabs.push(TabInfo {
        title: "b_sheet.snxsch".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    path
}

fn activate(app: &mut Signex, path: &std::path::Path) {
    app.document_state.active_tab = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    app.document_state.active_path = Some(path.to_path_buf());
}

/// Pick, panel Undo, then a different class edit lands at the same history
/// depth: Ctrl+Z must not revert that other edit.
#[test]
fn a_class_edit_at_the_same_depth_is_not_mistaken_for_the_pick() {
    let (mut app, _symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    panel(&mut app, NetClassMsg::Undo);
    panel(&mut app, NetClassMsg::Add);
    assert_eq!(class_count(&app), 2);

    app.handle_undo_requested();
    assert_eq!(class_count(&app), 2, "the new class stays");
    app.handle_redo_requested();
    assert_eq!(class_count(&app), 2, "redo does not touch it either");
}

/// The same sequence across two sheets: a pick on B takes the depth the
/// undone pick on A had, and Ctrl+Z on A must not revert B's pick.
#[test]
fn another_sheets_pick_at_the_same_depth_is_not_reverted_from_this_sheet() {
    let (mut app, _symbol) = app_with_sheet();
    let a = app.document_state.active_path.clone().unwrap();
    pick(&mut app, "GND", Some("Power"));
    panel(&mut app, NetClassMsg::Undo);
    open_second_sheet(&mut app);
    pick(&mut app, "VCC", Some("Power"));
    activate(&mut app, &a);

    app.handle_undo_requested();
    assert_eq!(nets(&app), ["VCC"], "B's pick is not undone from A");
}

/// At the history cap the depth stops growing; the pick must still be
/// recorded and undoable, and an older pick must not match later states.
#[test]
fn a_pick_made_at_the_history_cap_still_undoes() {
    let (mut app, _symbol) = app_with_sheet();
    for _ in 0..=signex_engine::NET_CLASS_HISTORY_LIMIT {
        panel(&mut app, NetClassMsg::Add);
    }
    pick(&mut app, "GND", Some("Power"));
    assert_eq!(nets(&app), ["GND"]);

    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the pick is reverted");
    app.handle_redo_requested();
    assert_eq!(nets(&app), ["GND"]);
}

/// A stale pick is dropped, and a still-valid older pick under it is used
/// instead of being skipped.
#[test]
fn a_stale_pick_is_dropped_and_the_older_valid_one_undone() {
    let (mut app, _symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    pick(&mut app, "VCC", Some("Power"));
    panel(&mut app, NetClassMsg::Undo);
    assert_eq!(nets(&app), ["GND"]);

    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the GND pick is undone");
}

#[test]
fn closing_a_sheet_forgets_its_picks() {
    let (mut app, _symbol) = app_with_sheet();
    let path = app.document_state.active_path.clone().unwrap();
    pick(&mut app, "GND", Some("Power"));
    assert!(app.has_net_class_pick(&path, true));
    app.forget_net_colors_of(&path);
    assert!(!app.has_net_class_pick(&path, true));
}
