//! #2267: a canvas menu row picked in an undocked window resolves through
//! `dispatch_command(…, Some(window))`. Every row that reads or edits the
//! sheet or board the window shows must still run in the window's swap, as
//! all rows did before that routing.

use super::*;
use crate::app::command::bridge::core_to_message;
use crate::app::command::{CommandArgs, InvocationContext};
use crate::keymap::AppCommandId;

/// The sheet menu's rows (and its Part Actions, Sheet Actions and Arrange
/// flyouts) that start from the shown sheet.
const SHEET_ROWS: &[&str] = &[
    "find",
    "find_similar_objects",
    "cut",
    "copy",
    "paste",
    "smart_paste",
    "duplicate",
    "select_all",
    "delete_selection",
    "rotate_counterclockwise",
    "mirror_x",
    "mirror_y",
    "highlight_net_under_cursor",
    "cross_probe",
    "open_datasheet",
    "edit_selected_symbol_in_symbol_editor",
    "change_component",
    "autoplace_fields",
    "copy_attributes_or_add_vertex",
    "open_child_sheet",
    "create_sheet_from_sheet_symbol",
    "sync_sheet_pins",
    "sync_sheet_entries",
    "create_sheet_symbol_from_sheet",
    "number_schematic_sheets",
    "bring_drawings_to_front",
    "send_drawings_to_back",
    "bring_drawings_forward",
    "send_drawings_backward",
];

/// The board menu's rows (and its Place flyout) that act on the shown board.
const BOARD_ROWS: &[&str] = &[
    "move_selection",
    "rotate_clockwise",
    "flip_selection",
    "change_footprint",
    "lock_board_items",
    "unlock_board_items",
    "unfill_selected",
    "swap_pins",
    "swap_gates",
    "save_board_reuse_block",
    "board_sketch",
    "gloss_selected_routes",
    "add_teardrops",
    "board_shape_from_selection",
    "keepout_from_selection",
    "arrange_in_box",
    "stitch_vias",
    "remove_teardrops",
    "tune_net_length",
    "tune_diff_pair",
    "repour_selected",
    "repour_all",
    "select_board_net",
    "highlight_board_net",
    "place_board_track",
    "place_board_via",
    "place_board_pad",
];

/// What the row for `id` sends when the menu was opened at a point.
fn menu_row_message(id: &str) -> Message {
    let command = AppCommandId::new(id).unwrap_or_else(|_| panic!("{id} is a catalog id"));
    let args = CommandArgs::in_context(InvocationContext {
        origin: Some((5.0, 5.0)),
        project: None,
    });
    core_to_message(&command, &args).unwrap_or_else(|| panic!("{id} resolves to a message"))
}

#[test]
fn canvas_menu_rows_that_use_the_shown_document_run_in_the_windows_swap() {
    for id in SHEET_ROWS.iter().chain(BOARD_ROWS) {
        let message = menu_row_message(id);
        assert!(
            command_runs_in_shown_document(&message),
            "{id} ({message:?}) would run on the main window's document"
        );
    }
}

/// Rows whose effect does not depend on the shown document: they run on the
/// main window and do the same there.
#[test]
fn app_wide_canvas_menu_rows_stay_on_the_main_window() {
    for id in [
        "open_preferences",
        "open_net_color_palette",
        "clear_net_highlighting",
        "edit_selected_object_properties",
    ] {
        let message = menu_row_message(id);
        assert!(
            !command_runs_in_shown_document(&message),
            "{id} ({message:?})"
        );
    }
}
