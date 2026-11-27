//! Mockup row 105: Layer Stack as a page tab of its board.

use std::path::{Path, PathBuf};

use signex_types::pcb::PcbBoard;
use signex_types::pcb_stack::StackLayerKind;
use uuid::Uuid;

use super::keep_pages_after_boards;
use crate::app::document::DocumentKey;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, OverlayMsg, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::layer_stack::{StackSubTab, StackUnit};
use crate::panels::{LayerStackMsg, PanelKind, PanelMsg, StackField};

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": [
            {"id": 0, "name": "TopCopper", "layer_type": "signal"},
            {"id": 31, "name": "BottomCopper", "layer_type": "signal"},
        ],
    }))
    .unwrap()
}

fn app_with(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn run_command(app: &mut Signex) {
    let id = AppCommandId::new("board_layer_stack").unwrap();
    let _ = app.update(Message::Menu(MenuMessage::Command(id)));
}

fn send(app: &mut Signex, msg: LayerStackMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::LayerStack(msg))));
}

fn pages(app: &Signex) -> usize {
    let tabs = &app.document_state.tabs;
    tabs.iter().filter(|t| t.kind.is_layer_stack()).count()
}

fn active_is_page(app: &Signex) -> bool {
    let docs = &app.document_state;
    docs.tabs[docs.active_tab].kind.is_layer_stack()
}

#[test]
fn the_command_opens_one_tab_after_the_board_and_focuses_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    assert!(active_is_page(&app));
    assert!(!app.is_panel_open(PanelKind::LayerStack), "no dock panel");
    let board_at = app
        .document_state
        .tabs
        .iter()
        .position(|t| matches!(t.kind, TabKind::Pcb))
        .unwrap();
    assert_eq!(app.document_state.active_tab, board_at + 1);
    assert_eq!(app.document_state.tabs[board_at + 1].path, path);

    app.document_state.active_tab = board_at;
    app.sync_active_tab();
    run_command(&mut app);
    assert_eq!(pages(&app), 1, "one tab per board");
    assert!(active_is_page(&app));
}

#[test]
fn the_panel_list_row_opens_the_tab_and_never_docks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path());
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPanel(
        PanelKind::LayerStack,
    )));
    assert_eq!(pages(&app), 1);
    assert!(
        app.document_view
            .dock
            .locate(PanelKind::LayerStack)
            .is_none()
    );
}

#[test]
fn an_edit_in_the_tab_dirties_and_undoes_on_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    let docs = &app.document_state;
    let key = DocumentKey::of(&docs.tabs[docs.active_tab]);
    assert_eq!(key, Some(DocumentKey::Pcb(path.clone())));

    send(
        &mut app,
        LayerStackMsg::LayerAdded {
            board: path.clone(),
            kind: StackLayerKind::Signal,
        },
    );
    assert!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .stackup
            .is_some()
    );
    assert!(app.document_state.is_document_dirty(&path));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .stackup
            .is_none()
    );
    assert!(active_is_page(&app), "undo stays on the tab");
}

#[test]
fn closing_the_tab_leaves_a_dirty_board_open_and_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    send(
        &mut app,
        LayerStackMsg::LayerAdded {
            board: path.clone(),
            kind: StackLayerKind::Plane,
        },
    );
    let page = app.document_state.active_tab;
    let _ = app.close_tab_parking_dirty(page);
    assert_eq!(pages(&app), 0);
    assert!(app.document_state.has_pcb_tab(&path), "the board stays");
    assert!(app.document_state.is_document_dirty(&path));
    assert!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .stackup
            .is_some()
    );
}

#[test]
fn closing_the_board_closes_its_tab_and_parks_the_unsaved_stack() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    send(
        &mut app,
        LayerStackMsg::LayerAdded {
            board: path.clone(),
            kind: StackLayerKind::Signal,
        },
    );
    let board_at = app
        .document_state
        .tabs
        .iter()
        .position(|t| matches!(t.kind, TabKind::Pcb))
        .unwrap();
    let _ = app.close_tab_parking_dirty(board_at);
    assert_eq!(pages(&app), 0);
    assert!(!app.document_state.has_pcb_tab(&path));
    let parked = app.document_state.pcb_board(&path).expect("parked board");
    assert!(parked.stackup.is_some(), "the edit is not lost");
}

#[test]
fn the_unit_reads_a_typed_thickness_and_the_board_stays_in_mm() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    send(
        &mut app,
        LayerStackMsg::UnitPicked {
            board: path.clone(),
            unit: StackUnit::Um,
        },
    );
    let shown = app.document_view.panel_ctx.layer_stack.clone().unwrap();
    let row = shown
        .stack
        .layers
        .iter()
        .position(|l| l.kind == StackLayerKind::Dielectric)
        .unwrap();
    send(
        &mut app,
        LayerStackMsg::FieldTyped {
            board: path.clone(),
            row,
            field: StackField::Thickness,
            text: "500".into(),
        },
    );
    send(
        &mut app,
        LayerStackMsg::FieldSubmitted {
            board: path.clone(),
            row,
            field: StackField::Thickness,
        },
    );
    let stored = app.document_state.pcb_board(&path).unwrap();
    let mm = stored.stackup.as_ref().unwrap().layers[row].thickness_mm;
    assert!((mm - 0.5).abs() < 1e-9, "{mm}");
}

#[test]
fn the_drill_sub_tab_is_picked_and_lists_the_through_pair() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    send(
        &mut app,
        LayerStackMsg::SubTabPicked {
            board: path,
            tab: StackSubTab::Drills,
        },
    );
    let shown = app.document_view.panel_ctx.layer_stack.clone().unwrap();
    assert_eq!(shown.sub_tab, StackSubTab::Drills);
    assert_eq!(shown.drill_pairs.len(), 1);
    assert_eq!(shown.drill_pairs[0].span, (1, 2));
}

fn tab(kind: TabKind, path: &str) -> TabInfo {
    crate::test_adapter::boardless_tab(String::new(), path, kind)
}

#[test]
fn a_tab_dragged_before_its_board_lands_just_after_it() {
    let mut tabs = vec![
        tab(TabKind::LayerStack, "/b.snxpcb"),
        tab(TabKind::Schematic, "/a.snxsch"),
        tab(TabKind::Pcb, "/b.snxpcb"),
        tab(TabKind::Schematic, "/c.snxsch"),
    ];
    let active = keep_pages_after_boards(&mut tabs, 0);
    assert!(matches!(tabs[1].kind, TabKind::Pcb));
    assert!(tabs[2].kind.is_layer_stack());
    assert_eq!(active, 2, "the dragged tab stays active");
}

#[test]
fn a_length_matching_tab_dragged_before_its_board_lands_just_after_it() {
    let mut tabs = vec![
        tab(TabKind::LengthMatching, "/b.snxpcb"),
        tab(TabKind::Schematic, "/a.snxsch"),
        tab(TabKind::Pcb, "/b.snxpcb"),
    ];
    let active = keep_pages_after_boards(&mut tabs, 0);
    assert!(matches!(tabs[1].kind, TabKind::Pcb));
    assert!(tabs[2].kind.is_length_matching());
    assert_eq!(active, 2, "the dragged tab stays active");
}

#[test]
fn the_active_tab_follows_when_a_page_moves_past_it() {
    let mut tabs = vec![
        tab(TabKind::LayerStack, "/b.snxpcb"),
        tab(TabKind::Schematic, "/a.snxsch"),
        tab(TabKind::Pcb, "/b.snxpcb"),
    ];
    let active = keep_pages_after_boards(&mut tabs, 1);
    assert_eq!(tabs[active].path, PathBuf::from("/a.snxsch"));
}
