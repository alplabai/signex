//! How Replay treats each catalog command that has no operands of its own.

/// The kind of edit a finished tool may hand to the row that started it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditFamily {
    Move,
    Properties,
    Union,
    Other,
}

/// How a command replays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::handlers) enum ReplayScope {
    /// Finishes inside its dispatch on the selection: its real handler runs
    /// again on the recorded targets.
    OnTargets,
    /// Makes a new selection from the targets: the result stays selected.
    SelectsFrom,
    /// Goes on after its dispatch (a tool, a dialog): the row takes the input
    /// of the edit that finishes it.
    Continues(ContinuesAs),
    /// Its effect is the selection itself: runs as it is.
    SetsSelection,
    /// Has no target: runs as it is, never seeing the current selection.
    NoTarget,
    /// Has nothing to repeat: see [`replay_scope`].
    NothingToRepeat,
}

/// Which finished edit a continuing command takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::handlers) enum ContinuesAs {
    Move,
    Union,
    Other,
}

impl ContinuesAs {
    pub(super) fn accepts(self, family: EditFamily) -> bool {
        matches!(
            (self, family),
            (Self::Move, EditFamily::Move)
                | (Self::Union, EditFamily::Union)
                | (Self::Other, EditFamily::Other | EditFamily::Properties)
        )
    }
}

/// Whether the board command `id` acts on the whole board when nothing is
/// selected, so an empty selection is its recorded input.
pub(super) fn acts_on_all_when_empty(id: &str) -> bool {
    matches!(id, "stitch_vias" | "add_teardrops" | "remove_teardrops")
}

/// The scope of the command `id`.
///
/// `NothingToRepeat` is for a step of a tool in progress (`remove_last_vertex`,
/// `undo_last_segment`), a pick at the pointer's position
/// (`select_node_or_connection_item`, `toggle_selection`), the Find Similar
/// dialog, whose result is a selection the person makes in it, and
/// `save_board_reuse_block` and `save_snippet`, which write a named file and
/// edit no document.
pub(in crate::app::handlers) fn replay_scope(id: &str) -> ReplayScope {
    if finishes_on_targets(id) {
        return ReplayScope::OnTargets;
    }
    match id {
        "select_connection" | "select_expand_connection" => ReplayScope::SelectsFrom,
        "select_all" | "unselect_all" => ReplayScope::SetsSelection,
        "move_selection" | "move_object" | "move_selection_xy" | "drag_keep_connections" => {
            ReplayScope::Continues(ContinuesAs::Move)
        }
        "create_union" | "add_to_union" | "remove_from_union" | "break_union" => {
            ReplayScope::Continues(ContinuesAs::Union)
        }
        "change_footprint"
        | "change_component"
        | "place_board_reuse_block"
        | "arrange_in_box"
        | "sync_sheet_entries"
        | "edit_blanket"
        | "edit_no_erc_rules" => ReplayScope::Continues(ContinuesAs::Other),
        "remove_last_vertex"
        | "undo_last_segment"
        | "select_node_or_connection_item"
        | "toggle_selection"
        | "find_similar_objects"
        | "save_board_reuse_block"
        | "save_snippet" => ReplayScope::NothingToRepeat,
        _ => ReplayScope::NoTarget,
    }
}

/// Whether `id` finishes inside its dispatch on the selection.
fn finishes_on_targets(id: &str) -> bool {
    matches!(
        id,
        "delete_selection"
            | "cut"
            | "copy"
            | "duplicate"
            | "rotate_clockwise"
            | "rotate_counterclockwise"
            | "rotate_selection"
            | "rotate_selection_cw"
            | "mirror_x"
            | "mirror_y"
            | "flip_selected_x"
            | "flip_selected_y"
            | "flip_selection"
            | "bring_drawings_to_front"
            | "send_drawings_to_back"
            | "bring_drawings_forward"
            | "send_drawings_backward"
            | "cross_probe"
            | "zoom_to_selection_area"
            | "sync_sheet_pins"
            | "open_child_sheet"
            | "create_sheet_from_sheet_symbol"
            | "open_datasheet"
            | "edit_library_symbol"
            | "edit_selected_symbol_in_symbol_editor"
            // Their handlers run again and mint new ids for what they add.
            | "stitch_vias"
            | "add_teardrops"
            | "remove_teardrops"
            | "gloss_selected_routes"
            | "board_shape_from_selection"
            | "keepout_from_selection"
            | "repour_selected"
            | "unfill_selected"
            | "tune_net_length"
            | "tune_diff_pair"
            | "lock_board_items"
            | "unlock_board_items"
            // One undo step per document, as the handler groups them.
            | "swap_pins"
            | "swap_gates"
    )
}
