//! The PCB panel through `Signex::update` (mockup rows 7 and 106).

use std::path::Path;

use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, OverlayMsg, Signex};
use crate::dock::DockMessage;
use crate::panels::{PanelKind, PanelMsg, PcbPanelMode, PcbPanelMsg};

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "VCC"}],
    }))
    .expect("a board")
}

fn app_on_board(dir: &Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.open_pcb_tab(dir.join("board.snxpcb"), "board".into(), board());
    app.refresh_panel_ctx();
    app
}

fn click(app: &mut Signex, net: &str) {
    let msg = PcbPanelMsg::NetClicked(net.to_owned());
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbPanel(msg))));
}

fn bright(app: &Signex) -> Option<String> {
    let query = &app.interaction_state.pcb_query;
    query.net_highlight.as_ref().map(|h| h.name.clone())
}

#[test]
fn a_net_click_selects_and_highlights_and_a_second_click_clears_both() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_on_board(dir.path());
    click(&mut app, "GND");
    let selected = || Some("GND".to_owned());
    assert_eq!(app.document_state.panel_ctx.pcb_panel.selected, selected());
    assert_eq!(bright(&app), selected());
    click(&mut app, "VCC");
    assert_eq!(bright(&app).as_deref(), Some("VCC"), "the highlight moves");
    click(&mut app, "VCC");
    assert_eq!(app.document_state.panel_ctx.pcb_panel.selected, None);
    assert_eq!(bright(&app), None);
}

#[test]
fn the_mode_survives_a_context_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_on_board(dir.path());
    let msg = PcbPanelMsg::ModePicked(PcbPanelMode::NetClasses);
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbPanel(msg))));
    app.refresh_panel_ctx();
    assert_eq!(
        app.document_state.panel_ctx.pcb_panel.mode,
        PcbPanelMode::NetClasses
    );
}

#[test]
fn net_classes_on_a_board_opens_the_pcb_panel_in_that_mode() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_on_board(dir.path());
    app.document_state.dock.close_panel(PanelKind::PcbNets);
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPanel(
        PanelKind::NetClasses,
    )));
    assert!(app.document_state.dock.locate(PanelKind::PcbNets).is_some());
    assert!(
        app.document_state
            .dock
            .locate(PanelKind::NetClasses)
            .is_none()
    );
    assert_eq!(
        app.document_state.panel_ctx.pcb_panel.mode,
        PcbPanelMode::NetClasses
    );
}
