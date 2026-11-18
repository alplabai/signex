//! Hand annotations of the parity table: what a route reads past its gate,
//! which the routing code does not say. Read from the handlers; a regenerate
//! (`UPDATE_GOLDEN=1`) keeps them, since they live here and not in the
//! golden. An empty list says the route was read and reads nothing implicit.
//! The grid and snap settings count as tool state.

use super::forms::Form::{self, ActiveBar, Bridge, BridgeContextMenu, HeadlessWithoutOperands};
use super::forms::Form::{Operand, StillRunsUnavailable};
use super::inputs::Input::{self, *};

/// `(ids, form, inputs)`: each id's row of `form` reads `inputs` too.
pub(super) const ANNOTATIONS: &[(&[&str], Form, &[Input])] = &[
    // Core edits and runs that read past their gate.
    (&["run_erc"], Bridge, &[RenderCache]),
    (
        &[
            "select_all",
            "zoom_to_fit",
            "zoom_to_all_objects",
            "show_all_design_objects",
        ],
        Bridge,
        &[RenderCache],
    ),
    (
        &["smart_paste", "paste_special"],
        Bridge,
        &[Clipboard, RenderCache],
    ),
    (&["paste"], Bridge, &[Clipboard]),
    (
        &["select_board_net", "highlight_board_net"],
        Bridge,
        &[Selection, Cursor],
    ),
    (
        &["select_board_net", "highlight_board_net"],
        Operand,
        &[Selection, Cursor],
    ),
    (
        &["tune_net_length", "tune_diff_pair"],
        Operand,
        &[ActiveDocument, Selection],
    ),
    (
        &["save_document_as", "close_active_document"],
        Operand,
        &[ActiveDocument],
    ),
    (&["new_document", "open_document"], Bridge, &[DialogAnswer]),
    (
        &["new_document", "open_document", "new_project_with_profile"],
        Operand,
        &[],
    ),
    (
        &["new_pcb_document", "new_schematic_sheet"],
        HeadlessWithoutOperands,
        &[ActiveDocument],
    ),
    (
        &["apply_selection_filter_preset", "set_selection_filters"],
        Operand,
        &[],
    ),
    (
        &[
            "align_bottom",
            "align_horizontal_centers",
            "align_left",
            "align_right",
            "align_to_grid",
            "align_top",
            "align_vertical_centers",
            "distribute_horizontally",
            "distribute_vertically",
            "bring_to_front",
            "move_to_front",
            "send_to_back",
        ],
        ActiveBar,
        &[ActiveDocument, Selection],
    ),
    (&["unselect_all"], Bridge, &[Selection]),
    // The placement handlers: a point must be where a click lands (grid,
    // and for a wire the electrical grid on the sheet in view); what the
    // operands leave out comes from the pre-placement form.
    (
        &["place_wire", "draw_wire"],
        Operand,
        &[ToolState, RenderCache],
    ),
    (&["place_bus", "draw_bus"], Operand, &[ToolState]),
    (
        &[
            "draw_line",
            "draw_graphic_line",
            "draw_rectangle",
            "place_net_label",
            "place_local_net_label",
            "place_global_label",
            "place_hierarchical_label",
        ],
        Operand,
        &[PrePlacementForm, ToolState],
    ),
    (
        &[
            "place_power_symbol",
            "place_no_connect",
            "place_junction",
            "place_no_erc",
            "place_text",
            "place_text_string",
            "place_blanket",
            "edit_blanket",
            "place_compile_mask",
            "place_diff_pair",
            "place_component",
            "place_sheet_symbol",
            "place_sheet_entry",
            "place_design_block",
            "import_graphics",
        ],
        Operand,
        &[ToolState],
    ),
    // Move Keeps Wires.
    (
        &["set_object_position", "set_object_rotation"],
        Operand,
        &[ToolState],
    ),
    (&["add_board_graphic"], Operand, &[PrePlacementForm]),
    // GUI routes.
    (&["find_similar_objects"], Bridge, &[Cursor, RenderCache]),
    (&["find_similar_objects"], BridgeContextMenu, &[RenderCache]),
    (
        &["cancel_current_tool"],
        Bridge,
        &[ToolState, InProgressEdit],
    ),
    (
        &["center_on_cursor", "center_view_at_cursor"],
        Bridge,
        &[Cursor, Window],
    ),
    (
        &[
            "change_component",
            "edit_library_symbol",
            "edit_selected_symbol_in_symbol_editor",
            "open_datasheet",
            "create_sheet_from_sheet_symbol",
            "open_child_sheet",
            "sync_sheet_pins",
            "drag_keep_connections",
            "go_to_net_partner",
            "highlight_related_net_objects",
        ],
        Bridge,
        &[Selection],
    ),
    (&["autoplace_fields"], Bridge, &[ActiveDocument, Selection]),
    (&["clear_all_net_colors"], ActiveBar, &[ActiveDocument]),
    (&["clear_net_color", "color_net"], ActiveBar, &[Selection]),
    (
        &[
            "bring_to_front_of",
            "send_to_back_of",
            "select_connection",
            "move_selection_xy",
            "drag_selection",
        ],
        ActiveBar,
        &[Selection],
    ),
    (&["select_expand_connection"], Bridge, &[Selection]),
    (
        &[
            "drag",
            "lasso_select",
            "toggle_selection",
            "tool_select",
            "place_component",
        ],
        ActiveBar,
        &[],
    ),
    (
        &[
            "copy_attributes_or_add_vertex",
            "highlight_net_under_cursor",
        ],
        Bridge,
        &[Cursor],
    ),
    (
        &[
            "copy_attributes_or_add_vertex",
            "highlight_net_under_cursor",
        ],
        BridgeContextMenu,
        &[ActiveDocument],
    ),
    (
        &[
            "edit_value",
            "edit_reference_designator",
            "edit_footprint_field",
            "edit_text_in_place",
        ],
        Bridge,
        &[Cursor, Selection],
    ),
    (
        &["select_node_or_connection_item"],
        Bridge,
        &[Cursor, RenderCache],
    ),
    (
        &["placement_accept"],
        Bridge,
        &[InProgressEdit, RenderCache],
    ),
    (
        &["zoom_to_selection_area"],
        Bridge,
        &[Selection, RenderCache],
    ),
    (
        &[
            "cycle_fast_grid",
            "cycle_snap_grid_backward",
            "previous_grid",
            "next_grid",
            "fast_grid_1",
            "fast_grid_2",
            "toggle_visible_grid",
            "toggle_electrical_grid",
            "cycle_unit",
            "cycle_selection_mode",
            "cycle_wire_bus_graphic_mode",
            "placement_properties",
        ],
        Bridge,
        &[ToolState],
    ),
    (
        &["switch_segment_posture"],
        Bridge,
        &[ToolState, InProgressEdit],
    ),
    (
        &[
            "footprint_mode_pads",
            "footprint_mode_sketch",
            "footprint_mode_view_3d",
            "footprint_recompute_courtyard_outline",
            "next_sheet",
            "previous_sheet",
            "open_project_options",
        ],
        Bridge,
        &[ActiveDocument],
    ),
    (
        &[
            "find_next",
            "find_previous",
            "clear_net_highlighting",
            "next_highlighted_net_item",
            "previous_highlighted_net_item",
            "next_document_tab",
            "previous_document_tab",
            "sheet_navigation_back",
            "sheet_navigation_forward",
            "toggle_auto_focus",
            "toggle_floating_panels",
            "toggle_search_panel",
            "toggle_layers_panel",
            "toggle_properties_panel",
            "toggle_schematic_filter_panel",
            "toggle_schematic_list_panel",
        ],
        Bridge,
        &[Window],
    ),
    // Dialogs, pickers and panels that open on nothing implicit, and the
    // tools that are only armed.
    (
        &[
            "annotate_schematic",
            "reset_schematic_designators",
            "find",
            "find_text",
            "find_and_replace",
            "open_command_palette",
            "open_grid_properties",
            "open_net_color_palette",
            "open_net_inspector",
            "open_preferences",
            "open_schematic_preferences",
            "show_current_command_hotkeys",
            "show_current_command_shortcuts",
            "new_project_with_profile",
            "toggle_cross_select_mode",
            "place_wire_to_bus_entry",
        ],
        Bridge,
        &[],
    ),
    (&["open_project_options"], BridgeContextMenu, &[]),
    // Planned Active Bar tools: they only say they are not built yet.
    (PLANNED, ActiveBar, &[]),
    (PLANNED, StillRunsUnavailable, &[]),
];

const PLANNED: &[&str] = &[
    "draw_bezier",
    "draw_ellipse",
    "draw_elliptical_arc",
    "draw_round_rectangle",
    "place_device_sheet_symbol",
    "place_graphic",
    "place_harness_connector",
    "place_harness_entry",
    "place_parameter_set",
    "place_reuse_block",
    "place_signal_harness",
];

/// The hand annotation of `(id, form)`, if any.
pub(super) fn of(id: &str, form: Form) -> Option<&'static [Input]> {
    ANNOTATIONS
        .iter()
        .find(|(ids, annotated, _)| *annotated == form && ids.contains(&id))
        .map(|(_, _, inputs)| *inputs)
}
