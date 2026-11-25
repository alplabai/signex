//! #2514 — with one board shown in the main window and in an undocked
//! window, a tool armed in one window dims only that window's canvas; a
//! highlighted net dims both.

use crate::app::state::pcb_query::BoardNetHighlight;
use crate::app::{InputMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvas;

use super::pcb_placement_window_tests::{Rig, menu_in_window, rig};

fn nets(canvas: &PcbCanvas) -> Vec<u32> {
    canvas
        .active_renderer_snapshot()
        .and_then(|snapshot| snapshot.paint.net_focus())
        .map(|focus| focus.nets.clone())
        .unwrap_or_default()
}

fn main_nets(rig: &Rig) -> Vec<u32> {
    nets(&rig.app.interaction_state.pcb_canvas)
}

fn window_nets(rig: &Rig) -> Vec<u32> {
    nets(&rig.app.interaction_state.pcb_canvases[&rig.window])
}

/// Board A in both windows, the Via tool placing on net 3.
fn both_windows_on_a() -> Rig {
    let mut rig = rig();
    let a = rig.a.clone();
    let index = rig.app.document_state.tabs.iter().position(|t| t.path == a);
    rig.app.document_state.active_tab = index.expect("board A is open");
    rig.app.sync_pcb_canvas_from_visible_board();
    rig.app.interaction_state.pcb_tool_defaults.via_net = Some(3);
    assert!(main_nets(&rig).is_empty() && window_nets(&rig).is_empty());
    rig
}

fn escape(app: &mut Signex, window: Option<iced::window::Id>) {
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window }));
}

#[test]
fn a_tool_in_the_undocked_window_dims_only_that_window() {
    let mut rig = both_windows_on_a();
    menu_in_window(&mut rig.app, rig.window, "place_board_via");
    assert_eq!(window_nets(&rig), [3], "the window with the tool dims");
    assert!(main_nets(&rig).is_empty(), "the main window does not");

    escape(&mut rig.app, Some(rig.window));
    assert!(rig.app.interaction_state.pcb_tool.is_none());
    assert!(window_nets(&rig).is_empty() && main_nets(&rig).is_empty());
}

#[test]
fn a_tool_in_the_main_window_dims_only_the_main_window() {
    let mut rig = both_windows_on_a();
    let id = AppCommandId::new("place_board_via").expect("a catalog id");
    let _ = rig.app.update(Message::Menu(MenuMessage::Command(id)));
    assert_eq!(main_nets(&rig), [3], "the main window with the tool dims");
    assert!(window_nets(&rig).is_empty(), "the undocked window does not");

    escape(&mut rig.app, None);
    assert!(rig.app.interaction_state.pcb_tool.is_none());
    assert!(main_nets(&rig).is_empty() && window_nets(&rig).is_empty());
}

#[test]
fn a_highlighted_net_dims_every_window_showing_the_board() {
    let mut rig = both_windows_on_a();
    rig.app.interaction_state.pcb_query.net_highlight = Some(BoardNetHighlight {
        path: rig.a.clone(),
        net: 5,
        name: "5".into(),
    });
    rig.app.refresh_pcb_highlight();
    assert_eq!(main_nets(&rig), [5]);
    assert_eq!(window_nets(&rig), [5]);

    // A tool in the window does not take the highlight's place there.
    menu_in_window(&mut rig.app, rig.window, "place_board_via");
    assert_eq!(window_nets(&rig), [5]);

    rig.app.interaction_state.pcb_query.net_highlight = None;
    rig.app.refresh_pcb_highlight();
    assert!(main_nets(&rig).is_empty(), "the main window has no tool");
    assert_eq!(window_nets(&rig), [3], "the window keeps its tool's net");
}
