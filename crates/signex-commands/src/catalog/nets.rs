//! Net commands: highlight a net and step through its items, jump to a net
//! partner, net colours, the Net Inspector and net class assignment.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const NETS: &[CommandMetadata] = &[
    CommandMetadata {
        id: "highlight_net_under_cursor",
        category: "select",
        label: command_ids::CMD_HIGHLIGHT_NET_UNDER_CURSOR,
        menu_label: Some(command_ids::CMD_MENU_HIGHLIGHT_NET_UNDER_CURSOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_related_net_objects",
        category: "select",
        label: command_ids::CMD_HIGHLIGHT_RELATED_NET_OBJECTS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_net_highlighting",
        category: "select",
        label: command_ids::CMD_CLEAR_NET_HIGHLIGHTING,
        menu_label: Some(command_ids::CMD_MENU_CLEAR_NET_HIGHLIGHTING),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_highlighted_net_item",
        category: "select",
        label: command_ids::CMD_NEXT_HIGHLIGHTED_NET_ITEM,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_highlighted_net_item",
        category: "select",
        label: command_ids::CMD_PREVIOUS_HIGHLIGHTED_NET_ITEM,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "go_to_net_partner",
        category: "navigation",
        label: command_ids::CMD_GO_TO_NET_PARTNER,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "color_net",
        category: "view",
        label: command_ids::CMD_COLOR_NET,
        menu_label: Some(command_ids::CMD_MENU_COLOR_NET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::COLOR_NET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_net_color",
        category: "view",
        label: command_ids::CMD_CLEAR_NET_COLOR,
        menu_label: Some(command_ids::CMD_MENU_CLEAR_NET_COLOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_all_net_colors",
        category: "view",
        label: command_ids::CMD_CLEAR_ALL_NET_COLORS,
        menu_label: Some(command_ids::CMD_MENU_CLEAR_ALL_NET_COLORS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_net_color_palette",
        category: "view",
        label: command_ids::CMD_OPEN_NET_COLOR_PALETTE,
        menu_label: Some(command_ids::CMD_MENU_OPEN_NET_COLOR_PALETTE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
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
        id: "assign_net_class",
        category: "design",
        label: command_ids::CMD_ASSIGN_NET_CLASS,
        menu_label: Some(command_ids::CMD_MENU_ASSIGN_NET_CLASS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::ASSIGN_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    // #2416: Design ▸ Net Classes opens the panel of the project's net-class
    // table, on a sheet or a board (`open_net_classes_panel` is its alias;
    // both keys work in those two contexts). No default key.
    CommandMetadata {
        id: "open_net_classes",
        category: "design",
        label: command_ids::CMD_OPEN_NET_CLASSES,
        menu_label: Some(command_ids::CMD_MENU_OPEN_NET_CLASSES),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2400: the Net Classes panel and the edits of its table, each one step
    // of the project's net-class history. `open_net_classes_panel` is an
    // alias of `open_net_classes`: the same panel on a sheet or a board.
    // No default keys.
    CommandMetadata {
        id: "open_net_classes_panel",
        category: "design",
        label: command_ids::CMD_OPEN_NET_CLASSES_PANEL,
        menu_label: Some(command_ids::CMD_MENU_OPEN_NET_CLASSES_PANEL),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "add_net_class",
        category: "design",
        label: command_ids::CMD_ADD_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::ADD_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rename_net_class",
        category: "design",
        label: command_ids::CMD_RENAME_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::RENAME_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "delete_net_class",
        category: "design",
        label: command_ids::CMD_DELETE_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_net_class_rules",
        category: "design",
        label: command_ids::CMD_SET_NET_CLASS_RULES,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::NET_CLASS_RULES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "assign_board_net_class",
        category: "design",
        label: command_ids::CMD_ASSIGN_BOARD_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::ASSIGN_BOARD_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    // #2403: the board net `net` names, else that of the selection or of
    // what is under the cursor: select its tracks, vias and pads, or draw
    // it bright (again: no longer). `net` comes from a typed command line
    // (the CLI), so neither is GUI-only. View and selection only; no key.
    CommandMetadata {
        id: "select_board_net",
        category: "select",
        label: command_ids::CMD_SELECT_BOARD_NET,
        menu_label: Some(command_ids::CMD_MENU_SELECT_BOARD_NET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_NET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_board_net",
        category: "select",
        label: command_ids::CMD_HIGHLIGHT_BOARD_NET,
        menu_label: Some(command_ids::CMD_MENU_HIGHLIGHT_BOARD_NET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_NET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "query_board_nets",
        category: "search",
        label: command_ids::CMD_QUERY_BOARD_NETS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::QUERY_BOARD_NETS,
        ..CommandMetadata::DEFAULT
    },
];
