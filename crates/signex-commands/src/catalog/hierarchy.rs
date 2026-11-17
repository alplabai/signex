//! Sheet hierarchy commands: sheet symbols and entries, child sheets,
//! sheet numbering and navigation between the sheets of a project.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const HIERARCHY: &[CommandMetadata] = &[
    CommandMetadata {
        id: "draw_hierarchical_sheet",
        category: "place",
        label: command_ids::CMD_DRAW_HIERARCHICAL_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_sheet_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_SHEET_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SHEET_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SHEET_SYMBOL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_sheet_entry",
        category: "place",
        label: command_ids::CMD_PLACE_SHEET_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SHEET_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SHEET_ENTRY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_device_sheet_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_DEVICE_SHEET_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_DEVICE_SHEET_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "create_sheet_from_sheet_symbol",
        category: "design",
        label: command_ids::CMD_CREATE_SHEET_FROM_SHEET_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_CREATE_SHEET_FROM_SHEET_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "sync_sheet_pins",
        category: "design",
        label: command_ids::CMD_SYNC_SHEET_PINS,
        menu_label: Some(command_ids::CMD_MENU_SYNC_SHEET_PINS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        operands: operands::SYNC_SHEET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "number_schematic_sheets",
        category: "design",
        label: command_ids::CMD_NUMBER_SCHEMATIC_SHEETS,
        menu_label: Some(command_ids::CMD_MENU_NUMBER_SCHEMATIC_SHEETS),
        group: CommandGroup::Schematic,
        // A project sheet, not the active tab, is what it needs: the root
        // sheet stands in with no document open, so the gate is
        // `Needs::ProjectSheet` (`app/command/needs.rs`).
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_child_sheet",
        category: "navigation",
        label: command_ids::CMD_OPEN_CHILD_SHEET,
        menu_label: Some(command_ids::CMD_MENU_OPEN_CHILD_SHEET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "leave_sheet",
        category: "navigation",
        label: command_ids::CMD_LEAVE_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "navigate_up_hierarchy",
        category: "navigation",
        label: command_ids::CMD_NAVIGATE_UP_HIERARCHY,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_sheet",
        category: "navigation",
        label: command_ids::CMD_NEXT_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_sheet",
        category: "navigation",
        label: command_ids::CMD_PREVIOUS_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "sheet_navigation_back",
        category: "navigation",
        label: command_ids::CMD_SHEET_NAVIGATION_BACK,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "sheet_navigation_forward",
        category: "navigation",
        label: command_ids::CMD_SHEET_NAVIGATION_FORWARD,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_sheet_symbol_size",
        category: "edit",
        label: command_ids::CMD_SET_SHEET_SYMBOL_SIZE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SHEET_SYMBOL_SIZE,
        ..CommandMetadata::DEFAULT
    },
];
