//! Wiring commands: wires, buses and their entries, net labels, ports,
//! off-sheet connectors and signal harnesses.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const WIRING: &[CommandMetadata] = &[
    CommandMetadata {
        id: "draw_wire",
        category: "place",
        label: command_ids::CMD_DRAW_WIRE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_WIRE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::WIRE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_wire",
        category: "place",
        label: command_ids::CMD_PLACE_WIRE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_WIRE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::WIRE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_bus",
        category: "place",
        label: command_ids::CMD_DRAW_BUS,
        menu_label: Some(command_ids::CMD_MENU_DRAW_BUS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::BUS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_bus",
        category: "place",
        label: command_ids::CMD_PLACE_BUS,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BUS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::BUS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_bus_entry",
        category: "place",
        label: command_ids::CMD_PLACE_BUS_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BUS_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_wire_to_bus_entry",
        category: "place",
        label: command_ids::CMD_PLACE_WIRE_TO_BUS_ENTRY,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
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
        id: "place_net_label",
        category: "place",
        label: command_ids::CMD_PLACE_NET_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NET_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_local_net_label",
        category: "place",
        label: command_ids::CMD_PLACE_LOCAL_NET_LABEL,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_global_label",
        category: "place",
        label: command_ids::CMD_PLACE_GLOBAL_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_GLOBAL_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_hierarchical_label",
        category: "place",
        label: command_ids::CMD_PLACE_HIERARCHICAL_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HIERARCHICAL_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_port",
        category: "place",
        label: command_ids::CMD_PLACE_PORT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_PORT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_off_sheet_connector",
        category: "place",
        label: command_ids::CMD_PLACE_OFF_SHEET_CONNECTOR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_OFF_SHEET_CONNECTOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_signal_harness",
        category: "place",
        label: command_ids::CMD_PLACE_SIGNAL_HARNESS,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SIGNAL_HARNESS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_connector",
        category: "place",
        label: command_ids::CMD_PLACE_HARNESS_CONNECTOR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HARNESS_CONNECTOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_entry",
        category: "place",
        label: command_ids::CMD_PLACE_HARNESS_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HARNESS_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
