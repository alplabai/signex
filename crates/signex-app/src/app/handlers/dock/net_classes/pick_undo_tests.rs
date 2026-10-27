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
        dirty: false,
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
            .dirty_paths
            .contains(&app.document_state.projects[0].path)
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
