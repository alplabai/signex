use crate::keymap::AppCommandId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMetadata {
    pub id: &'static str,
    pub category: &'static str,
    pub label: &'static str,
}

pub const COMMAND_METADATA: &[CommandMetadata] = &[
    CommandMetadata {
        id: "annotate_schematic",
        category: "design",
        label: "Annotate schematic",
    },
    CommandMetadata {
        id: "autoplace_fields",
        category: "edit",
        label: "Autoplace fields",
    },
    CommandMetadata {
        id: "break_wire",
        category: "modify",
        label: "Break wire",
    },
    CommandMetadata {
        id: "cancel_current_tool",
        category: "interactive",
        label: "Cancel current stage / exit placement mode",
    },
    CommandMetadata {
        id: "center_on_cursor",
        category: "view",
        label: "Center on cursor",
    },
    CommandMetadata {
        id: "center_view_at_cursor",
        category: "view",
        label: "Center/redraw view around cursor",
    },
    CommandMetadata {
        id: "clear_net_highlighting",
        category: "select",
        label: "Clear net highlighting",
    },
    CommandMetadata {
        id: "close_active_document",
        category: "file",
        label: "Close active document",
    },
    CommandMetadata {
        id: "copy",
        category: "edit",
        label: "Copy",
    },
    CommandMetadata {
        id: "copy_attributes_or_add_vertex",
        category: "interactive",
        label: "Copy attributes from object under cursor or add vertex while drawing",
    },
    CommandMetadata {
        id: "cut",
        category: "edit",
        label: "Cut",
    },
    CommandMetadata {
        id: "cycle_fast_grid",
        category: "view",
        label: "Cycle fast grid",
    },
    CommandMetadata {
        id: "cycle_snap_grid_backward",
        category: "view",
        label: "Cycle backward through snap grids",
    },
    CommandMetadata {
        id: "cycle_snap_grid_forward",
        category: "view",
        label: "Cycle forward through snap grids",
    },
    CommandMetadata {
        id: "cycle_wire_bus_graphic_mode",
        category: "interactive",
        label: "Cycle wire/bus/graphic-line mode: free, 90 degrees, 45 degrees",
    },
    CommandMetadata {
        id: "cycle_wiring_mode",
        category: "interactive",
        label: "Change wiring mode while dragging connected electrical objects",
    },
    CommandMetadata {
        id: "delete_selection",
        category: "edit",
        label: "Delete selection",
    },
    CommandMetadata {
        id: "drag_keep_connections",
        category: "modify",
        label: "Drag while keeping connections",
    },
    CommandMetadata {
        id: "draw_graphic_line",
        category: "place",
        label: "Draw graphic line",
    },
    CommandMetadata {
        id: "draw_hierarchical_sheet",
        category: "place",
        label: "Draw hierarchical sheet",
    },
    CommandMetadata {
        id: "duplicate",
        category: "edit",
        label: "Duplicate",
    },
    CommandMetadata {
        id: "edit_footprint_field",
        category: "edit",
        label: "Edit footprint field",
    },
    CommandMetadata {
        id: "edit_library_symbol",
        category: "library",
        label: "Edit library symbol",
    },
    CommandMetadata {
        id: "edit_object_properties",
        category: "edit",
        label: "Edit properties of object under cursor",
    },
    CommandMetadata {
        id: "edit_reference_designator",
        category: "edit",
        label: "Edit reference designator",
    },
    CommandMetadata {
        id: "edit_selected_object_properties",
        category: "edit",
        label: "Properties / edit selected object",
    },
    CommandMetadata {
        id: "edit_selected_symbol_in_symbol_editor",
        category: "library",
        label: "Edit selected symbol in Symbol Editor",
    },
    CommandMetadata {
        id: "edit_text_in_place",
        category: "edit",
        label: "In-place edit selected text",
    },
    CommandMetadata {
        id: "edit_value",
        category: "edit",
        label: "Edit value",
    },
    CommandMetadata {
        id: "fast_grid_1",
        category: "view",
        label: "Fast grid 1",
    },
    CommandMetadata {
        id: "fast_grid_2",
        category: "view",
        label: "Fast grid 2",
    },
    CommandMetadata {
        id: "find",
        category: "search",
        label: "Find",
    },
    CommandMetadata {
        id: "find_and_replace",
        category: "search",
        label: "Find and replace",
    },
    CommandMetadata {
        id: "find_next",
        category: "search",
        label: "Find next",
    },
    CommandMetadata {
        id: "find_previous",
        category: "search",
        label: "Find previous",
    },
    CommandMetadata {
        id: "find_similar_objects",
        category: "select",
        label: "Find Similar Objects",
    },
    CommandMetadata {
        id: "find_text",
        category: "search",
        label: "Find text",
    },
    CommandMetadata {
        id: "highlight_net_under_cursor",
        category: "select",
        label: "Highlight net under cursor",
    },
    CommandMetadata {
        id: "highlight_related_net_objects",
        category: "select",
        label: "Highlight/select related net objects across sheets",
    },
    CommandMetadata {
        id: "import_graphics",
        category: "library",
        label: "Import graphics",
    },
    CommandMetadata {
        id: "leave_sheet",
        category: "navigation",
        label: "Leave sheet / go to parent sheet",
    },
    CommandMetadata {
        id: "measure_distance",
        category: "view",
        label: "Measure distance",
    },
    CommandMetadata {
        id: "mirror_x",
        category: "modify",
        label: "Mirror along X-axis",
    },
    CommandMetadata {
        id: "mirror_y",
        category: "modify",
        label: "Mirror along Y-axis",
    },
    CommandMetadata {
        id: "move_object",
        category: "modify",
        label: "Move object",
    },
    CommandMetadata {
        id: "move_selection",
        category: "modify",
        label: "Move",
    },
    CommandMetadata {
        id: "navigate_up_hierarchy",
        category: "navigation",
        label: "Navigate up hierarchy",
    },
    CommandMetadata {
        id: "new_document",
        category: "file",
        label: "New",
    },
    CommandMetadata {
        id: "next_document_tab",
        category: "window",
        label: "Next open document tab",
    },
    CommandMetadata {
        id: "next_grid",
        category: "view",
        label: "Next grid",
    },
    CommandMetadata {
        id: "next_highlighted_net_item",
        category: "select",
        label: "Next item on highlighted net",
    },
    CommandMetadata {
        id: "next_sheet",
        category: "navigation",
        label: "Next sheet",
    },
    CommandMetadata {
        id: "open_components_panel",
        category: "panels",
        label: "Open Components panel / place components",
    },
    CommandMetadata {
        id: "open_datasheet",
        category: "edit",
        label: "Open datasheet",
    },
    CommandMetadata {
        id: "open_document",
        category: "file",
        label: "Open document",
    },
    CommandMetadata {
        id: "open_schematic_preferences",
        category: "preferences",
        label: "Open schematic preferences",
    },
    CommandMetadata {
        id: "paste",
        category: "edit",
        label: "Paste",
    },
    CommandMetadata {
        id: "paste_special",
        category: "edit",
        label: "Paste special",
    },
    CommandMetadata {
        id: "place_bus",
        category: "place",
        label: "Draw bus",
    },
    CommandMetadata {
        id: "place_compile_mask",
        category: "place",
        label: "Place Compile Mask directive",
    },
    CommandMetadata {
        id: "place_design_block",
        category: "place",
        label: "Place design block",
    },
    CommandMetadata {
        id: "place_global_label",
        category: "place",
        label: "Place global label",
    },
    CommandMetadata {
        id: "place_hierarchical_label",
        category: "place",
        label: "Place hierarchical label",
    },
    CommandMetadata {
        id: "place_junction",
        category: "place",
        label: "Place junction",
    },
    CommandMetadata {
        id: "place_local_net_label",
        category: "place",
        label: "Place local net label",
    },
    CommandMetadata {
        id: "place_net_label",
        category: "place",
        label: "Place net label",
    },
    CommandMetadata {
        id: "place_no_connect",
        category: "place",
        label: "Place no-connect flag",
    },
    CommandMetadata {
        id: "place_no_erc",
        category: "place",
        label: "Place Generic No ERC directive",
    },
    CommandMetadata {
        id: "place_power_symbol",
        category: "place",
        label: "Place power symbol",
    },
    CommandMetadata {
        id: "place_symbol",
        category: "place",
        label: "Place symbol",
    },
    CommandMetadata {
        id: "place_text",
        category: "place",
        label: "Place text",
    },
    CommandMetadata {
        id: "place_wire",
        category: "place",
        label: "Place wire",
    },
    CommandMetadata {
        id: "place_wire_to_bus_entry",
        category: "place",
        label: "Place wire-to-bus entry",
    },
    CommandMetadata {
        id: "placement_accept",
        category: "interactive",
        label: "Accept current placement or move stage",
    },
    CommandMetadata {
        id: "placement_properties",
        category: "interactive",
        label: "Edit properties of the object being placed or moved",
    },
    CommandMetadata {
        id: "previous_document_tab",
        category: "window",
        label: "Previous open document tab",
    },
    CommandMetadata {
        id: "previous_grid",
        category: "view",
        label: "Previous grid",
    },
    CommandMetadata {
        id: "previous_highlighted_net_item",
        category: "select",
        label: "Previous item on highlighted net",
    },
    CommandMetadata {
        id: "previous_sheet",
        category: "navigation",
        label: "Previous sheet",
    },
    CommandMetadata {
        id: "print",
        category: "file",
        label: "Print",
    },
    CommandMetadata {
        id: "redo",
        category: "edit",
        label: "Redo",
    },
    CommandMetadata {
        id: "refresh_view",
        category: "view",
        label: "Refresh/redraw",
    },
    CommandMetadata {
        id: "remove_last_vertex",
        category: "interactive",
        label: "Remove last placed vertex while drawing",
    },
    CommandMetadata {
        id: "repeat_last_item",
        category: "edit",
        label: "Repeat last item",
    },
    CommandMetadata {
        id: "report_manager_bom",
        category: "reports",
        label: "Report Manager / BOM",
    },
    CommandMetadata {
        id: "reset_local_coordinates",
        category: "view",
        label: "Reset local coordinates",
    },
    CommandMetadata {
        id: "reset_schematic_designators",
        category: "design",
        label: "Reset schematic designators",
    },
    CommandMetadata {
        id: "rotate_clockwise",
        category: "modify",
        label: "Rotate clockwise by 90 degrees",
    },
    CommandMetadata {
        id: "rotate_counterclockwise",
        category: "modify",
        label: "Rotate counterclockwise by 90 degrees",
    },
    CommandMetadata {
        id: "rubber_stamp_copy",
        category: "edit",
        label: "Rubber-stamp copy / repeated paste",
    },
    CommandMetadata {
        id: "save_document",
        category: "file",
        label: "Save document",
    },
    CommandMetadata {
        id: "save_document_as",
        category: "file",
        label: "Save as",
    },
    CommandMetadata {
        id: "select_all",
        category: "select",
        label: "Select all",
    },
    CommandMetadata {
        id: "select_expand_connection",
        category: "select",
        label: "Select / expand connection",
    },
    CommandMetadata {
        id: "select_node_or_connection_item",
        category: "select",
        label: "Select node / connection item under cursor",
    },
    CommandMetadata {
        id: "sheet_navigation_back",
        category: "navigation",
        label: "Sheet navigation back",
    },
    CommandMetadata {
        id: "sheet_navigation_forward",
        category: "navigation",
        label: "Sheet navigation forward",
    },
    CommandMetadata {
        id: "show_all_design_objects",
        category: "view",
        label: "Show all design objects",
    },
    CommandMetadata {
        id: "show_current_command_hotkeys",
        category: "help",
        label: "Show graphical editing hotkey list for current command",
    },
    CommandMetadata {
        id: "show_current_command_shortcuts",
        category: "help",
        label: "Show valid shortcuts for the current interactive command",
    },
    CommandMetadata {
        id: "smart_paste",
        category: "edit",
        label: "Smart Paste",
    },
    CommandMetadata {
        id: "switch_segment_posture",
        category: "interactive",
        label: "Switch current segment posture",
    },
    CommandMetadata {
        id: "toggle_cross_select_mode",
        category: "select",
        label: "Toggle Cross Select Mode",
    },
    CommandMetadata {
        id: "toggle_electrical_grid",
        category: "view",
        label: "Toggle electrical grid",
    },
    CommandMetadata {
        id: "toggle_floating_panels",
        category: "window",
        label: "Toggle floating panels",
    },
    CommandMetadata {
        id: "toggle_properties_panel",
        category: "panels",
        label: "Toggle Properties panel",
    },
    CommandMetadata {
        id: "toggle_schematic_filter_panel",
        category: "panels",
        label: "Toggle schematic filter panel",
    },
    CommandMetadata {
        id: "toggle_schematic_list_panel",
        category: "panels",
        label: "Toggle schematic list panel",
    },
    CommandMetadata {
        id: "toggle_search_panel",
        category: "search",
        label: "Show/hide search panel",
    },
    CommandMetadata {
        id: "toggle_selection",
        category: "select",
        label: "Add/remove object from selection",
    },
    CommandMetadata {
        id: "toggle_visible_grid",
        category: "view",
        label: "Toggle visible grid",
    },
    CommandMetadata {
        id: "undo",
        category: "edit",
        label: "Undo",
    },
    CommandMetadata {
        id: "undo_last_segment",
        category: "interactive",
        label: "Undo last segment while drawing",
    },
    CommandMetadata {
        id: "unselect_all",
        category: "select",
        label: "Unselect all",
    },
    CommandMetadata {
        id: "update_pcb_from_schematic",
        category: "pcb_sync",
        label: "Update PCB from schematic",
    },
    CommandMetadata {
        id: "zoom_in_at_cursor",
        category: "view",
        label: "Zoom in at cursor",
    },
    CommandMetadata {
        id: "zoom_out_at_cursor",
        category: "view",
        label: "Zoom out at cursor",
    },
    CommandMetadata {
        id: "zoom_to_all_objects",
        category: "view",
        label: "Zoom to all objects",
    },
    CommandMetadata {
        id: "zoom_to_fit",
        category: "view",
        label: "Zoom to fit",
    },
    CommandMetadata {
        id: "zoom_to_selection_area",
        category: "view",
        label: "Zoom to selection area",
    },
];

pub fn metadata_for(command: &AppCommandId) -> Option<&'static CommandMetadata> {
    COMMAND_METADATA
        .iter()
        .find(|metadata| metadata.id == command.as_str())
}

pub fn fallback_label(command: &AppCommandId) -> String {
    command
        .as_str()
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut label = first.to_ascii_uppercase().to_string();
                    label.push_str(chars.as_str());
                    label
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
