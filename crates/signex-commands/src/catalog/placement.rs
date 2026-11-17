//! Placement commands: components, symbols, power ports, junctions and
//! markers, blocks, directives and text on a schematic sheet.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const PLACEMENT: &[CommandMetadata] = &[
    CommandMetadata {
        id: "place_component",
        category: "place",
        label: command_ids::CMD_PLACE_COMPONENT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_COMPONENT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::PART,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_SYMBOL,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_power_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_POWER_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_POWER_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::POWER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_junction",
        category: "place",
        label: command_ids::CMD_PLACE_JUNCTION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_JUNCTION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::JUNCTION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_no_connect",
        category: "place",
        label: command_ids::CMD_PLACE_NO_CONNECT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::AT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_no_erc",
        category: "place",
        label: command_ids::CMD_PLACE_NO_ERC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NO_ERC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::AT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_compile_mask",
        category: "place",
        label: command_ids::CMD_PLACE_COMPILE_MASK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_COMPILE_MASK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_design_block",
        category: "place",
        label: command_ids::CMD_PLACE_DESIGN_BLOCK,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_reuse_block",
        category: "place",
        label: command_ids::CMD_PLACE_REUSE_BLOCK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_REUSE_BLOCK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_parameter_set",
        category: "place",
        label: command_ids::CMD_PLACE_PARAMETER_SET,
        menu_label: Some(command_ids::CMD_MENU_PLACE_PARAMETER_SET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_diff_pair",
        category: "place",
        label: command_ids::CMD_PLACE_DIFF_PAIR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_DIFF_PAIR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_blanket",
        category: "place",
        label: command_ids::CMD_PLACE_BLANKET,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BLANKET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_string",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT_STRING,
        menu_label: Some(command_ids::CMD_MENU_PLACE_TEXT_STRING),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_frame",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT_FRAME,
        menu_label: Some(command_ids::CMD_MENU_PLACE_TEXT_FRAME),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_note",
        category: "place",
        label: command_ids::CMD_PLACE_NOTE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NOTE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_graphic",
        category: "place",
        label: command_ids::CMD_PLACE_GRAPHIC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_GRAPHIC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
