//! General-surface command metadata — shared editing / file / view /
//! transform commands, plus anything without an obvious home. Split
//! from `keymap/catalog.rs`; entries verbatim.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n::command_ids;

pub(super) const GENERAL: &[CommandMetadata] = &[
    CommandMetadata {
        id: "apply_edits",
        category: "edit",
        label: command_ids::CMD_APPLY_EDITS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::APPLY_EDITS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "autoplace_fields",
        category: "edit",
        label: command_ids::CMD_AUTOPLACE_FIELDS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bring_drawings_forward",
        category: "edit",
        label: command_ids::CMD_BRING_DRAWINGS_FORWARD,
        menu_label: Some(command_ids::CMD_MENU_BRING_DRAWINGS_FORWARD),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bring_drawings_to_front",
        category: "edit",
        label: command_ids::CMD_BRING_DRAWINGS_TO_FRONT,
        menu_label: Some(command_ids::CMD_MENU_BRING_DRAWINGS_TO_FRONT),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_drawings_backward",
        category: "edit",
        label: command_ids::CMD_SEND_DRAWINGS_BACKWARD,
        menu_label: Some(command_ids::CMD_MENU_SEND_DRAWINGS_BACKWARD),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_drawings_to_back",
        category: "edit",
        label: command_ids::CMD_SEND_DRAWINGS_TO_BACK,
        menu_label: Some(command_ids::CMD_MENU_SEND_DRAWINGS_TO_BACK),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "break_wire",
        category: "modify",
        label: command_ids::CMD_BREAK_WIRE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cancel_current_tool",
        category: "interactive",
        label: command_ids::CMD_CANCEL_CURRENT_TOOL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "center_on_cursor",
        category: "view",
        label: command_ids::CMD_CENTER_ON_CURSOR,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "center_view_at_cursor",
        category: "view",
        label: command_ids::CMD_CENTER_VIEW_AT_CURSOR,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "check_drc",
        category: "validation",
        label: command_ids::CMD_CHECK_DRC,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::CHECK_DRC,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "check_erc",
        category: "validation",
        label: command_ids::CMD_CHECK_ERC,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::CHECK_ERC,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "close_active_document",
        category: "file",
        label: command_ids::CMD_CLOSE_ACTIVE_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_CLOSE_ACTIVE_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "close_all_documents",
        category: "window",
        label: command_ids::CMD_CLOSE_ALL_DOCUMENTS,
        menu_label: Some(command_ids::CMD_MENU_CLOSE_ALL_DOCUMENTS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "copy",
        category: "edit",
        label: command_ids::CMD_COPY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::RUNS_HEADLESS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "copy_attributes_or_add_vertex",
        category: "interactive",
        label: command_ids::CMD_COPY_ATTRIBUTES_OR_ADD_VERTEX,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #1312: shows the selection's counterpart on the other side (sheet or
    // board), opening it when needed. No default key in any preset.
    CommandMetadata {
        id: "cross_probe",
        category: "select",
        label: command_ids::CMD_CROSS_PROBE,
        menu_label: Some(command_ids::CMD_MENU_CROSS_PROBE),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cut",
        category: "edit",
        label: command_ids::CMD_CUT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_fast_grid",
        category: "view",
        label: command_ids::CMD_CYCLE_FAST_GRID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_unit",
        category: "view",
        label: command_ids::CMD_CYCLE_UNIT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_snap_grid_backward",
        category: "view",
        label: command_ids::CMD_CYCLE_SNAP_GRID_BACKWARD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_selection_mode",
        category: "select",
        label: command_ids::CMD_CYCLE_SELECTION_MODE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_snap_grid_forward",
        category: "view",
        // The key opens the snap-grid picker (Altium's G popup, footprint
        // editor only); it does not step through sizes (#525).
        label: command_ids::CMD_CYCLE_SNAP_GRID_FORWARD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_wire_bus_graphic_mode",
        category: "interactive",
        label: command_ids::CMD_CYCLE_WIRE_BUS_GRAPHIC_MODE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_wiring_mode",
        category: "interactive",
        label: command_ids::CMD_CYCLE_WIRING_MODE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "delete_selection",
        category: "edit",
        label: command_ids::CMD_DELETE_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_DELETE_SELECTION),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "document_info",
        category: "file",
        label: command_ids::CMD_DOCUMENT_INFO,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "drag_keep_connections",
        category: "modify",
        label: command_ids::CMD_DRAG_KEEP_CONNECTIONS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "duplicate",
        category: "edit",
        label: command_ids::CMD_DUPLICATE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_footprint_field",
        category: "edit",
        label: command_ids::CMD_EDIT_FOOTPRINT_FIELD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_library_symbol",
        category: "library",
        label: command_ids::CMD_EDIT_LIBRARY_SYMBOL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_object_properties",
        category: "edit",
        label: command_ids::CMD_EDIT_OBJECT_PROPERTIES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_reference_designator",
        category: "edit",
        label: command_ids::CMD_EDIT_REFERENCE_DESIGNATOR,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_selected_object_properties",
        category: "edit",
        label: command_ids::CMD_EDIT_SELECTED_OBJECT_PROPERTIES,
        menu_label: Some(command_ids::CMD_MENU_EDIT_SELECTED_OBJECT_PROPERTIES),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_selected_symbol_in_symbol_editor",
        category: "library",
        label: command_ids::CMD_EDIT_SELECTED_SYMBOL_IN_SYMBOL_EDITOR,
        menu_label: Some(command_ids::CMD_MENU_EDIT_SELECTED_SYMBOL_IN_SYMBOL_EDITOR),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_text_in_place",
        category: "edit",
        label: command_ids::CMD_EDIT_TEXT_IN_PLACE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_value",
        category: "edit",
        label: command_ids::CMD_EDIT_VALUE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "fast_grid_1",
        category: "view",
        label: command_ids::CMD_FAST_GRID_1,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "fast_grid_2",
        category: "view",
        label: command_ids::CMD_FAST_GRID_2,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find",
        category: "search",
        label: command_ids::CMD_FIND,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_items",
        category: "search",
        label: command_ids::CMD_FIND_ITEMS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::FIND,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_and_replace",
        category: "search",
        label: command_ids::CMD_FIND_AND_REPLACE,
        menu_label: Some(command_ids::CMD_MENU_FIND_AND_REPLACE),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_next",
        category: "search",
        label: command_ids::CMD_FIND_NEXT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_previous",
        category: "search",
        label: command_ids::CMD_FIND_PREVIOUS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_similar_objects",
        category: "select",
        label: command_ids::CMD_FIND_SIMILAR_OBJECTS,
        menu_label: Some(command_ids::CMD_MENU_FIND_SIMILAR_OBJECTS),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_text",
        category: "search",
        label: command_ids::CMD_FIND_TEXT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "import_graphics",
        category: "library",
        label: command_ids::CMD_IMPORT_GRAPHICS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "list_documents",
        category: "file",
        label: command_ids::CMD_LIST_DOCUMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT_FILTERS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "measure_distance",
        category: "view",
        label: command_ids::CMD_MEASURE_DISTANCE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "mirror_x",
        category: "modify",
        label: command_ids::CMD_MIRROR_X,
        menu_label: Some(command_ids::CMD_MENU_MIRROR_X),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "mirror_y",
        category: "modify",
        label: command_ids::CMD_MIRROR_Y,
        menu_label: Some(command_ids::CMD_MENU_MIRROR_Y),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_object",
        category: "modify",
        label: command_ids::CMD_MOVE_OBJECT,
        menu_label: Some(command_ids::CMD_MENU_MOVE_OBJECT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_selection",
        category: "modify",
        label: command_ids::CMD_MOVE_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_MOVE_SELECTION),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "new_document",
        category: "file",
        label: command_ids::CMD_NEW_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_NEW_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SAVE_PATH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "new_project_with_profile",
        category: "file",
        label: command_ids::CMD_NEW_PROJECT_WITH_PROFILE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NEW_PROJECT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_document_tab",
        category: "window",
        label: command_ids::CMD_NEXT_DOCUMENT_TAB,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_grid",
        category: "view",
        label: command_ids::CMD_NEXT_GRID,
        menu_label: Some(command_ids::CMD_MENU_NEXT_GRID),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_command_palette",
        category: "commands",
        label: command_ids::CMD_OPEN_COMMAND_PALETTE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_datasheet",
        category: "edit",
        label: command_ids::CMD_OPEN_DATASHEET,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DATASHEET),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_document",
        category: "file",
        label: command_ids::CMD_OPEN_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::OPEN_PATH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_grid_picker",
        category: "view",
        label: command_ids::CMD_OPEN_GRID_PICKER,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_grid_properties",
        category: "view",
        label: command_ids::CMD_OPEN_GRID_PROPERTIES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_library_browser",
        category: "library",
        label: command_ids::CMD_OPEN_LIBRARY_BROWSER,
        menu_label: Some(command_ids::CMD_MENU_OPEN_LIBRARY_BROWSER),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_net_inspector",
        category: "panels",
        label: command_ids::CMD_OPEN_NET_INSPECTOR,
        menu_label: Some(command_ids::CMD_MENU_OPEN_NET_INSPECTOR),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_preferences",
        category: "preferences",
        label: command_ids::CMD_OPEN_PREFERENCES,
        menu_label: Some(command_ids::CMD_MENU_OPEN_PREFERENCES),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_schematic_preferences",
        category: "preferences",
        label: command_ids::CMD_OPEN_SCHEMATIC_PREFERENCES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "paste",
        category: "edit",
        label: command_ids::CMD_PASTE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "paste_array",
        category: "edit",
        label: command_ids::CMD_PASTE_ARRAY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "paste_special",
        category: "edit",
        label: command_ids::CMD_PASTE_SPECIAL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "placement_accept",
        category: "interactive",
        label: command_ids::CMD_PLACEMENT_ACCEPT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "placement_properties",
        category: "interactive",
        label: command_ids::CMD_PLACEMENT_PROPERTIES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_document_tab",
        category: "window",
        label: command_ids::CMD_PREVIOUS_DOCUMENT_TAB,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_grid",
        category: "view",
        label: command_ids::CMD_PREVIOUS_GRID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "print",
        category: "file",
        label: command_ids::CMD_PRINT,
        menu_label: Some(command_ids::CMD_MENU_PRINT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "query_pins",
        category: "file",
        label: command_ids::CMD_QUERY_PINS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::QUERY_PINS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "read_document",
        category: "file",
        label: command_ids::CMD_READ_DOCUMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::READ_DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "redo",
        category: "edit",
        label: command_ids::CMD_REDO,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "refresh_view",
        category: "view",
        label: command_ids::CMD_REFRESH_VIEW,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "remove_last_vertex",
        category: "interactive",
        label: command_ids::CMD_REMOVE_LAST_VERTEX,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "repeat_last_item",
        category: "edit",
        label: command_ids::CMD_REPEAT_LAST_ITEM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "report_manager_bom",
        category: "reports",
        label: command_ids::CMD_REPORT_MANAGER_BOM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "reset_local_coordinates",
        category: "view",
        label: command_ids::CMD_RESET_LOCAL_COORDINATES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "resolve",
        category: "file",
        label: command_ids::CMD_RESOLVE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::RESOLVE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "revert",
        category: "edit",
        label: command_ids::CMD_REVERT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: operands::REVERT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_clockwise",
        category: "modify",
        label: command_ids::CMD_ROTATE_CLOCKWISE,
        menu_label: Some(command_ids::CMD_MENU_ROTATE_CLOCKWISE),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_counterclockwise",
        category: "modify",
        label: command_ids::CMD_ROTATE_COUNTERCLOCKWISE,
        menu_label: Some(command_ids::CMD_MENU_ROTATE_COUNTERCLOCKWISE),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rubber_stamp_copy",
        category: "edit",
        label: command_ids::CMD_RUBBER_STAMP_COPY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_document",
        category: "file",
        label: command_ids::CMD_SAVE_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_SAVE_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_document_as",
        category: "file",
        label: command_ids::CMD_SAVE_DOCUMENT_AS,
        menu_label: Some(command_ids::CMD_MENU_SAVE_DOCUMENT_AS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SAVE_DOCUMENT_AS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "select_all",
        category: "select",
        label: command_ids::CMD_SELECT_ALL,
        menu_label: Some(command_ids::CMD_MENU_SELECT_ALL),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::RUNS_HEADLESS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "select_expand_connection",
        category: "select",
        label: command_ids::CMD_SELECT_EXPAND_CONNECTION,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "select_node_or_connection_item",
        category: "select",
        label: command_ids::CMD_SELECT_NODE_OR_CONNECTION_ITEM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_all_design_objects",
        category: "view",
        label: command_ids::CMD_SHOW_ALL_DESIGN_OBJECTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_current_command_hotkeys",
        category: "help",
        label: command_ids::CMD_SHOW_CURRENT_COMMAND_HOTKEYS,
        menu_label: Some(command_ids::CMD_MENU_SHOW_CURRENT_COMMAND_HOTKEYS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_current_command_shortcuts",
        category: "help",
        label: command_ids::CMD_SHOW_CURRENT_COMMAND_SHORTCUTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "smart_paste",
        category: "edit",
        label: command_ids::CMD_SMART_PASTE,
        menu_label: Some(command_ids::CMD_MENU_SMART_PASTE),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "switch_segment_posture",
        category: "interactive",
        label: command_ids::CMD_SWITCH_SEGMENT_POSTURE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "tile_windows_horizontally",
        category: "window",
        label: command_ids::CMD_TILE_WINDOWS_HORIZONTALLY,
        menu_label: Some(command_ids::CMD_MENU_TILE_WINDOWS_HORIZONTALLY),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "tile_windows_vertically",
        category: "window",
        label: command_ids::CMD_TILE_WINDOWS_VERTICALLY,
        menu_label: Some(command_ids::CMD_MENU_TILE_WINDOWS_VERTICALLY),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_cross_select_mode",
        category: "select",
        label: command_ids::CMD_TOGGLE_CROSS_SELECT_MODE,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_CROSS_SELECT_MODE),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_electrical_grid",
        category: "view",
        label: command_ids::CMD_TOGGLE_ELECTRICAL_GRID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_floating_panels",
        category: "window",
        label: command_ids::CMD_TOGGLE_FLOATING_PANELS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_properties_panel",
        category: "panels",
        label: command_ids::CMD_TOGGLE_PROPERTIES_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_schematic_filter_panel",
        category: "panels",
        label: command_ids::CMD_TOGGLE_SCHEMATIC_FILTER_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_schematic_list_panel",
        category: "panels",
        label: command_ids::CMD_TOGGLE_SCHEMATIC_LIST_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_search_panel",
        category: "search",
        label: command_ids::CMD_TOGGLE_SEARCH_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_selection",
        category: "select",
        label: command_ids::CMD_TOGGLE_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_SELECTION),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_auto_focus",
        category: "view",
        label: command_ids::CMD_TOGGLE_AUTO_FOCUS,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_AUTO_FOCUS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_visible_grid",
        category: "view",
        label: command_ids::CMD_TOGGLE_VISIBLE_GRID,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_VISIBLE_GRID),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "undo",
        category: "edit",
        label: command_ids::CMD_UNDO,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "undo_last_segment",
        category: "interactive",
        label: command_ids::CMD_UNDO_LAST_SEGMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "unselect_all",
        category: "select",
        label: command_ids::CMD_UNSELECT_ALL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::RUNS_HEADLESS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_in_at_cursor",
        category: "view",
        label: command_ids::CMD_ZOOM_IN_AT_CURSOR,
        menu_label: Some(command_ids::CMD_MENU_ZOOM_IN_AT_CURSOR),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_out_at_cursor",
        category: "view",
        label: command_ids::CMD_ZOOM_OUT_AT_CURSOR,
        menu_label: Some(command_ids::CMD_MENU_ZOOM_OUT_AT_CURSOR),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_to_all_objects",
        category: "view",
        label: command_ids::CMD_ZOOM_TO_ALL_OBJECTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_to_fit",
        category: "view",
        label: command_ids::CMD_ZOOM_TO_FIT,
        menu_label: Some(command_ids::CMD_MENU_ZOOM_TO_FIT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_to_selection_area",
        category: "view",
        label: command_ids::CMD_ZOOM_TO_SELECTION_AREA,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
