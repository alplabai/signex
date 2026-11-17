//! Net commands: highlight a net and step through its items, jump to a net
//! partner, net colours, the Net Inspector and net class assignment.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const NETS: &[CommandMetadata] = &[
    CommandMetadata {
        id: "highlight_net_under_cursor",
        category: "select",
        label: command_ids::CMD_HIGHLIGHT_NET_UNDER_CURSOR,
        menu_label: None,
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
        menu_label: None,
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
        menu_label: None,
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
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::ASSIGN_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
];
