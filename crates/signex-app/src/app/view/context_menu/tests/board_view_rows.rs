//! #2240 / #2223: the board canvas menu ends with a View group whose
//! Toggle Ratsnest row runs `toggle_ratsnest`, with or without a selection.

use signex_widgets::active_bar_dropdown::DropdownEntry;

use super::{TID, row_message, shortcuts};
use crate::app::handlers::pcb_placement::PcbTargets;
use crate::app::view::context_menu::menus::pcb_canvas_menu_entries;
use crate::app::{ContextAction, ContextMenuMsg, Message};

fn runs_toggle_ratsnest(message: Option<Message>) -> bool {
    matches!(
        message,
        Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command { id, project: None })))
            if id.as_str() == "toggle_ratsnest"
    )
}

#[test]
fn the_board_menu_ends_with_a_live_toggle_ratsnest_row() {
    let sc = shortcuts();
    for any in [false, true] {
        let board = PcbTargets {
            shown: true,
            any,
            ..PcbTargets::default()
        };
        let rows = pcb_canvas_menu_entries(TID, board, &sc);
        assert!(runs_toggle_ratsnest(row_message(&rows, "Toggle Ratsnest")));
        // Its own group: a separator above it, and it is the last row.
        let n = rows.len();
        assert!(matches!(rows[n - 2], DropdownEntry::Separator));
        assert!(matches!(&rows[n - 1], DropdownEntry::Item(it) if it.label == "Toggle Ratsnest"));
    }
}

/// #1312: the board menu's Properties row shows the Properties panel, live
/// with anything selected.
#[test]
fn the_board_menu_properties_row_is_live_with_a_selection() {
    let sc = shortcuts();
    let picked = PcbTargets {
        shown: true,
        any: true,
        ..PcbTargets::default()
    };
    let rows = pcb_canvas_menu_entries(TID, picked, &sc);
    assert!(matches!(
        row_message(&rows, "Properties..."),
        Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command { id, project: None })))
            if id.as_str() == "edit_selected_object_properties"
    ));
    let empty = PcbTargets {
        shown: true,
        ..PcbTargets::default()
    };
    let rows = pcb_canvas_menu_entries(TID, empty, &sc);
    assert!(row_message(&rows, "Properties...").is_none());
}
