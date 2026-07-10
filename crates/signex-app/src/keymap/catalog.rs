use crate::keymap::AppCommandId;

/// Coarse editor-surface bucket used by the Keyboard Shortcuts pane to
/// group commands for display. Distinct from [`CommandMetadata::category`],
/// which stays fine-grained (place / edit / view …); the group is the
/// primary *surface* a command belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandGroup {
    /// Shared editing / file / view / transform commands available on
    /// every surface, plus anything without an obvious home.
    General,
    /// Schematic-specific placement, net, annotation and sheet-navigation
    /// commands.
    Schematic,
    /// PCB routing / layer / via / DRC commands.
    Pcb,
    /// Footprint-editor and 3D-view commands.
    ThreeD,
}

impl CommandGroup {
    /// Display order for the grouped Keyboard Shortcuts view.
    pub const ALL: &'static [CommandGroup] = &[
        CommandGroup::General,
        CommandGroup::Schematic,
        CommandGroup::Pcb,
        CommandGroup::ThreeD,
    ];

    /// Human-readable header shown above each group.
    pub fn display_name(&self) -> &'static str {
        match self {
            CommandGroup::General => "General",
            CommandGroup::Schematic => "Schematic",
            CommandGroup::Pcb => "PCB",
            CommandGroup::ThreeD => "3D",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMetadata {
    pub id: &'static str,
    pub category: &'static str,
    pub label: &'static str,
    pub group: CommandGroup,
}

pub const COMMAND_METADATA: &[CommandMetadata] = &[
    CommandMetadata {
        id: "annotate_schematic",
        category: "design",
        label: "Annotate schematic",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "annotate_schematic_quietly",
        category: "design",
        label: "Annotate schematic quietly",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "autoplace_fields",
        category: "edit",
        label: "Autoplace fields",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "break_wire",
        category: "modify",
        label: "Break wire",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cancel_current_tool",
        category: "interactive",
        label: "Cancel current stage / exit placement mode",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "center_on_cursor",
        category: "view",
        label: "Center on cursor",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "center_view_at_cursor",
        category: "view",
        label: "Center/redraw view around cursor",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "clear_net_highlighting",
        category: "select",
        label: "Clear net highlighting",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "close_active_document",
        category: "file",
        label: "Close active document",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "copy",
        category: "edit",
        label: "Copy",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "copy_attributes_or_add_vertex",
        category: "interactive",
        label: "Copy attributes from object under cursor or add vertex while drawing",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cut",
        category: "edit",
        label: "Cut",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cycle_fast_grid",
        category: "view",
        label: "Cycle fast grid",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cycle_unit",
        category: "view",
        label: "Cycle display unit",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cycle_snap_grid_backward",
        category: "view",
        label: "Cycle backward through snap grids",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cycle_selection_mode",
        category: "select",
        label: "Cycle selection mode",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cycle_snap_grid_forward",
        category: "view",
        label: "Cycle forward through snap grids",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cycle_wire_bus_graphic_mode",
        category: "interactive",
        label: "Cycle wire/bus/graphic-line mode: free, 90 degrees, 45 degrees",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "cycle_wiring_mode",
        category: "interactive",
        label: "Change wiring mode while dragging connected electrical objects",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "delete_selection",
        category: "edit",
        label: "Delete selection",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "drag_keep_connections",
        category: "modify",
        label: "Drag while keeping connections",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "draw_graphic_line",
        category: "place",
        label: "Draw graphic line",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "draw_hierarchical_sheet",
        category: "place",
        label: "Draw hierarchical sheet",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "duplicate",
        category: "edit",
        label: "Duplicate",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_footprint_field",
        category: "edit",
        label: "Edit footprint field",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_library_symbol",
        category: "library",
        label: "Edit library symbol",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_object_properties",
        category: "edit",
        label: "Edit properties of object under cursor",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_reference_designator",
        category: "edit",
        label: "Edit reference designator",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_selected_object_properties",
        category: "edit",
        label: "Properties / edit selected object",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_selected_symbol_in_symbol_editor",
        category: "library",
        label: "Edit selected symbol in Symbol Editor",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_text_in_place",
        category: "edit",
        label: "In-place edit selected text",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "edit_value",
        category: "edit",
        label: "Edit value",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "fast_grid_1",
        category: "view",
        label: "Fast grid 1",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "fast_grid_2",
        category: "view",
        label: "Fast grid 2",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "find",
        category: "search",
        label: "Find",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "find_and_replace",
        category: "search",
        label: "Find and replace",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "find_next",
        category: "search",
        label: "Find next",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "find_previous",
        category: "search",
        label: "Find previous",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "find_similar_objects",
        category: "select",
        label: "Find Similar Objects",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "find_text",
        category: "search",
        label: "Find text",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "footprint_mode_pads",
        category: "library",
        label: "Switch footprint editor to Pads mode",
        group: CommandGroup::ThreeD,
    },
    CommandMetadata {
        id: "footprint_mode_sketch",
        category: "library",
        label: "Switch footprint editor to Sketch mode",
        group: CommandGroup::ThreeD,
    },
    CommandMetadata {
        id: "footprint_mode_view_3d",
        category: "library",
        label: "Switch footprint editor to 3D View mode",
        group: CommandGroup::ThreeD,
    },
    CommandMetadata {
        id: "force_annotate_all_schematics",
        category: "design",
        label: "Force annotate all schematics",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "highlight_net_under_cursor",
        category: "select",
        label: "Highlight net under cursor",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "highlight_related_net_objects",
        category: "select",
        label: "Highlight/select related net objects across sheets",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "import_graphics",
        category: "library",
        label: "Import graphics",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "leave_sheet",
        category: "navigation",
        label: "Leave sheet / go to parent sheet",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "measure_distance",
        category: "view",
        label: "Measure distance",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "mirror_x",
        category: "modify",
        label: "Mirror along X-axis",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "mirror_y",
        category: "modify",
        label: "Mirror along Y-axis",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "move_object",
        category: "modify",
        label: "Move object",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "move_selection",
        category: "modify",
        label: "Move",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "navigate_up_hierarchy",
        category: "navigation",
        label: "Navigate up hierarchy",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "new_document",
        category: "file",
        label: "New",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "next_document_tab",
        category: "window",
        label: "Next open document tab",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "next_grid",
        category: "view",
        label: "Next grid",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "next_highlighted_net_item",
        category: "select",
        label: "Next item on highlighted net",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "next_sheet",
        category: "navigation",
        label: "Next sheet",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "open_components_panel",
        category: "panels",
        label: "Open Components panel / place components",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "open_command_palette",
        category: "commands",
        label: "Open command palette",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "open_datasheet",
        category: "edit",
        label: "Open datasheet",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "open_document",
        category: "file",
        label: "Open document",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "open_grid_picker",
        category: "view",
        label: "Open grid picker",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "open_grid_properties",
        category: "view",
        label: "Open grid properties",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "open_net_color_palette",
        category: "view",
        label: "Open net color palette",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "open_preferences",
        category: "preferences",
        label: "Open preferences",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "open_schematic_preferences",
        category: "preferences",
        label: "Open schematic preferences",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "paste",
        category: "edit",
        label: "Paste",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "paste_special",
        category: "edit",
        label: "Paste special",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "place_bus",
        category: "place",
        label: "Draw bus",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_compile_mask",
        category: "place",
        label: "Place Compile Mask directive",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_design_block",
        category: "place",
        label: "Place design block",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_global_label",
        category: "place",
        label: "Place global label",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_hierarchical_label",
        category: "place",
        label: "Place hierarchical label",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_junction",
        category: "place",
        label: "Place junction",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_local_net_label",
        category: "place",
        label: "Place local net label",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_net_label",
        category: "place",
        label: "Place net label",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_no_connect",
        category: "place",
        label: "Place no-connect flag",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_no_erc",
        category: "place",
        label: "Place Generic No ERC directive",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_power_symbol",
        category: "place",
        label: "Place power symbol",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_symbol",
        category: "place",
        label: "Place symbol",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_text",
        category: "place",
        label: "Place text",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_wire",
        category: "place",
        label: "Place wire",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "place_wire_to_bus_entry",
        category: "place",
        label: "Place wire-to-bus entry",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "placement_accept",
        category: "interactive",
        label: "Accept current placement or move stage",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "placement_properties",
        category: "interactive",
        label: "Edit properties of the object being placed or moved",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "previous_document_tab",
        category: "window",
        label: "Previous open document tab",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "previous_grid",
        category: "view",
        label: "Previous grid",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "previous_highlighted_net_item",
        category: "select",
        label: "Previous item on highlighted net",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "previous_sheet",
        category: "navigation",
        label: "Previous sheet",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "print",
        category: "file",
        label: "Print",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "redo",
        category: "edit",
        label: "Redo",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "refresh_view",
        category: "view",
        label: "Refresh/redraw",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "remove_last_vertex",
        category: "interactive",
        label: "Remove last placed vertex while drawing",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "repeat_last_item",
        category: "edit",
        label: "Repeat last item",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "report_manager_bom",
        category: "reports",
        label: "Report Manager / BOM",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "run_erc",
        category: "validation",
        label: "Run electrical rules check",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "reset_local_coordinates",
        category: "view",
        label: "Reset local coordinates",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "reset_schematic_designators",
        category: "design",
        label: "Reset schematic designators",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "rotate_clockwise",
        category: "modify",
        label: "Rotate clockwise by 90 degrees",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "rotate_counterclockwise",
        category: "modify",
        label: "Rotate counterclockwise by 90 degrees",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "rubber_stamp_copy",
        category: "edit",
        label: "Rubber-stamp copy / repeated paste",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "save_document",
        category: "file",
        label: "Save document",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "save_document_as",
        category: "file",
        label: "Save as",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "select_all",
        category: "select",
        label: "Select all",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "select_expand_connection",
        category: "select",
        label: "Select / expand connection",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "select_node_or_connection_item",
        category: "select",
        label: "Select node / connection item under cursor",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "sheet_navigation_back",
        category: "navigation",
        label: "Sheet navigation back",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "sheet_navigation_forward",
        category: "navigation",
        label: "Sheet navigation forward",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "show_all_design_objects",
        category: "view",
        label: "Show all design objects",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "show_current_command_hotkeys",
        category: "help",
        label: "Show graphical editing hotkey list for current command",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "show_current_command_shortcuts",
        category: "help",
        label: "Show valid shortcuts for the current interactive command",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "smart_paste",
        category: "edit",
        label: "Smart Paste",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "switch_segment_posture",
        category: "interactive",
        label: "Switch current segment posture",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_cross_select_mode",
        category: "select",
        label: "Toggle Cross Select Mode",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_electrical_grid",
        category: "view",
        label: "Toggle electrical grid",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_floating_panels",
        category: "window",
        label: "Toggle floating panels",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_properties_panel",
        category: "panels",
        label: "Toggle Properties panel",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_schematic_filter_panel",
        category: "panels",
        label: "Toggle schematic filter panel",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_schematic_list_panel",
        category: "panels",
        label: "Toggle schematic list panel",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_search_panel",
        category: "search",
        label: "Show/hide search panel",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_selection",
        category: "select",
        label: "Add/remove object from selection",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_auto_focus",
        category: "view",
        label: "Toggle AutoFocus",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "toggle_visible_grid",
        category: "view",
        label: "Toggle visible grid",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "undo",
        category: "edit",
        label: "Undo",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "undo_last_segment",
        category: "interactive",
        label: "Undo last segment while drawing",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "unselect_all",
        category: "select",
        label: "Unselect all",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "update_pcb_from_schematic",
        category: "pcb_sync",
        label: "Update PCB from schematic",
        group: CommandGroup::Schematic,
    },
    CommandMetadata {
        id: "zoom_in_at_cursor",
        category: "view",
        label: "Zoom in at cursor",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "zoom_out_at_cursor",
        category: "view",
        label: "Zoom out at cursor",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "zoom_to_all_objects",
        category: "view",
        label: "Zoom to all objects",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "zoom_to_fit",
        category: "view",
        label: "Zoom to fit",
        group: CommandGroup::General,
    },
    CommandMetadata {
        id: "zoom_to_selection_area",
        category: "view",
        label: "Zoom to selection area",
        group: CommandGroup::General,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn group_of(id: &str) -> CommandGroup {
        COMMAND_METADATA
            .iter()
            .find(|metadata| metadata.id == id)
            .unwrap_or_else(|| panic!("missing command metadata for `{id}`"))
            .group
    }

    #[test]
    fn every_command_has_a_group_in_display_order() {
        // Catches an entry that was left with a stray / unlisted group.
        for metadata in COMMAND_METADATA {
            assert!(
                CommandGroup::ALL.contains(&metadata.group),
                "command `{}` has a group absent from CommandGroup::ALL",
                metadata.id
            );
        }
    }

    #[test]
    fn command_groups_follow_primary_surface() {
        // Schematic: placement, design, ERC, PCB-sync, net, sheet-nav.
        assert_eq!(group_of("place_wire"), CommandGroup::Schematic);
        assert_eq!(group_of("place_symbol"), CommandGroup::Schematic);
        assert_eq!(group_of("run_erc"), CommandGroup::Schematic);
        assert_eq!(group_of("annotate_schematic"), CommandGroup::Schematic);
        assert_eq!(group_of("update_pcb_from_schematic"), CommandGroup::Schematic);
        assert_eq!(group_of("next_sheet"), CommandGroup::Schematic);
        assert_eq!(
            group_of("highlight_net_under_cursor"),
            CommandGroup::Schematic
        );
        assert_eq!(group_of("open_components_panel"), CommandGroup::Schematic);
        // Footprint editor / 3D view.
        assert_eq!(group_of("footprint_mode_pads"), CommandGroup::ThreeD);
        assert_eq!(group_of("footprint_mode_view_3d"), CommandGroup::ThreeD);
        // Shared editing / transform / view / file → General.
        assert_eq!(group_of("copy"), CommandGroup::General);
        assert_eq!(group_of("rotate_clockwise"), CommandGroup::General);
        assert_eq!(group_of("mirror_x"), CommandGroup::General);
        assert_eq!(group_of("zoom_to_fit"), CommandGroup::General);
        assert_eq!(group_of("open_preferences"), CommandGroup::General);
    }

    #[test]
    fn grouping_partitions_every_command_exactly_once() {
        let summed: usize = CommandGroup::ALL
            .iter()
            .map(|group| {
                COMMAND_METADATA
                    .iter()
                    .filter(|metadata| metadata.group == *group)
                    .count()
            })
            .sum();
        assert_eq!(
            summed,
            COMMAND_METADATA.len(),
            "each command must land in exactly one CommandGroup"
        );
        // The two primary EDA surfaces must carry commands.
        assert!(
            COMMAND_METADATA
                .iter()
                .any(|metadata| metadata.group == CommandGroup::Schematic)
        );
        assert!(
            COMMAND_METADATA
                .iter()
                .any(|metadata| metadata.group == CommandGroup::ThreeD)
        );
    }
}
