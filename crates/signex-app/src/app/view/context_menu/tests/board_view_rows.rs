//! #2240 / #2223: the board canvas menu ends with a View group whose
//! Toggle Ratsnest row runs `toggle_ratsnest`, with or without a selection.

use signex_widgets::active_bar_dropdown::DropdownEntry;

use super::{TID, row_message, shortcuts};
use crate::app::handlers::pcb_placement::PcbTargets;
use crate::app::view::context_menu::menus::{pcb_canvas_menu_entries, pcb_canvas_menu_with_blocks};
use crate::app::{ContextAction, ContextMenuMsg, Message, ReuseBlockMsg};

fn runs(message: Option<Message>, command: &str) -> bool {
    matches!(
        message,
        Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command { id, project: None })))
            if id.as_str() == command
    )
}

/// #2410: Swap Pins and Swap Gates run their commands with two pads of one
/// unlocked component selected, Save as Reuse Block with board objects
/// selected, and each project block gets a Place row live with one unlocked
/// component selected; all greyed otherwise, and the View group still ends
/// the menu.
#[test]
fn the_board_menu_swaps_pins_and_saves_and_places_reuse_blocks() {
    let sc = shortcuts();
    let i18n = signex_i18n::I18n::en_us_ref();
    let picked = PcbTargets {
        shown: true,
        any: true,
        placeable: true,
        copyable: true,
        pad_pair: true,
        one_footprint: true,
        ..PcbTargets::default()
    };
    let blocks = vec!["amp".to_string()];
    let rows = pcb_canvas_menu_with_blocks(i18n, TID, picked, &sc, &blocks);
    assert!(runs(row_message(&rows, "Swap Pins"), "swap_pins"));
    assert!(runs(row_message(&rows, "Swap Gates"), "swap_gates"));
    assert!(runs(
        row_message(&rows, "Save as Reuse Block"),
        "save_board_reuse_block"
    ));
    let place = rows.iter().find_map(|row| match row {
        DropdownEntry::Item(it) if it.label.contains("amp") => Some(it.on_press.clone()),
        _ => None,
    });
    assert!(
        matches!(
            &place,
            Some(Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::ReuseBlock(
                ReuseBlockMsg::PlaceRequested { name }
            )))))
                if name == "amp"
        ),
        "{place:?}"
    );
    assert!(
        matches!(&rows[rows.len() - 1], DropdownEntry::Item(it) if it.label == "Toggle Ratsnest")
    );

    let empty = PcbTargets {
        shown: true,
        ..PcbTargets::default()
    };
    let rows = pcb_canvas_menu_with_blocks(i18n, TID, empty, &sc, &[]);
    assert!(row_message(&rows, "Swap Pins").is_none());
    assert!(row_message(&rows, "Save as Reuse Block").is_none());
    assert!(
        row_message(&rows, "Place Reuse Block").is_none(),
        "greyed with no block"
    );
}

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
        let rows = pcb_canvas_menu_entries(signex_i18n::I18n::en_us_ref(), TID, board, &sc);
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
    let rows = pcb_canvas_menu_entries(signex_i18n::I18n::en_us_ref(), TID, picked, &sc);
    assert!(matches!(
        row_message(&rows, "Properties..."),
        Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command { id, project: None })))
            if id.as_str() == "edit_selected_object_properties"
    ));
    let empty = PcbTargets {
        shown: true,
        ..PcbTargets::default()
    };
    let rows = pcb_canvas_menu_entries(signex_i18n::I18n::en_us_ref(), TID, empty, &sc);
    assert!(row_message(&rows, "Properties...").is_none());
}

/// #2366: Change Footprint... runs `change_footprint` with an unlocked
/// footprint selected, and is greyed otherwise.
#[test]
fn the_board_menu_change_footprint_row_needs_an_unlocked_footprint() {
    let sc = shortcuts();
    let placeable = PcbTargets {
        shown: true,
        any: true,
        placeable: true,
        ..PcbTargets::default()
    };
    let rows = pcb_canvas_menu_entries(signex_i18n::I18n::en_us_ref(), TID, placeable, &sc);
    assert!(matches!(
        row_message(&rows, "Change Footprint..."),
        Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command { id, project: None })))
            if id.as_str() == "change_footprint"
    ));
    let locked = PcbTargets {
        shown: true,
        any: true,
        ..PcbTargets::default()
    };
    let rows = pcb_canvas_menu_entries(signex_i18n::I18n::en_us_ref(), TID, locked, &sc);
    assert!(row_message(&rows, "Change Footprint...").is_none());
    assert!(
        rows.iter()
            .any(|row| matches!(row, DropdownEntry::Item(it) if it.label == "Change Footprint...")),
        "the row is there, greyed"
    );
}
