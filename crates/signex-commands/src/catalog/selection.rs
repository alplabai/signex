//! Selection commands: the select tools, select and unselect all, the
//! selection modes, connection selection and cross-probing.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const SELECTION: &[CommandMetadata] = &[
    CommandMetadata {
        id: "tool_select",
        category: "select",
        label: command_ids::CMD_TOOL_SELECT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "lasso_select",
        category: "select",
        label: command_ids::CMD_LASSO_SELECT,
        menu_label: Some(command_ids::CMD_MENU_LASSO_SELECT),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "select_connection",
        category: "select",
        label: command_ids::CMD_SELECT_CONNECTION,
        menu_label: Some(command_ids::CMD_MENU_SELECT_CONNECTION),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
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
        id: "find_similar_objects",
        category: "select",
        label: command_ids::CMD_FIND_SIMILAR_OBJECTS,
        menu_label: Some(command_ids::CMD_MENU_FIND_SIMILAR_OBJECTS),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // Sheet selection, scoped like `select_connection` (#1668).
    CommandMetadata {
        id: "select_expand_connection",
        category: "select",
        label: command_ids::CMD_SELECT_EXPAND_CONNECTION,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "select_node_or_connection_item",
        category: "select",
        label: command_ids::CMD_SELECT_NODE_OR_CONNECTION_ITEM,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #1312: the selection's counterpart on the other side; keys since #2416.
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
        id: "toggle_cross_select_mode",
        category: "select",
        label: command_ids::CMD_TOGGLE_CROSS_SELECT_MODE,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_CROSS_SELECT_MODE),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2417: the schematic Selection Filter (Filter panel, Active Bar
    // dropdown) from its operands; window state, so no document.
    CommandMetadata {
        id: "set_selection_filters",
        category: "select",
        label: command_ids::CMD_SET_SELECTION_FILTERS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SELECTION_FILTER_KINDS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "apply_selection_filter_preset",
        category: "select",
        label: command_ids::CMD_APPLY_SELECTION_FILTER_PRESET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SELECTION_FILTER_PRESET,
        ..CommandMetadata::DEFAULT
    },
    // #2416: View ▸ Selection Filter opens the Active Bar's filter of the
    // sheet. No default key.
    CommandMetadata {
        id: "open_selection_filter",
        category: "select",
        label: command_ids::CMD_OPEN_SELECTION_FILTER,
        menu_label: Some(command_ids::CMD_MENU_OPEN_SELECTION_FILTER),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
