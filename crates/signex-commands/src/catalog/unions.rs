//! Named-union commands (#158): create a union from the selection, add the
//! selection to it or take it out, and break it. Schematic only.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const UNIONS: &[CommandMetadata] = &[
    CommandMetadata {
        id: "create_union",
        category: "edit",
        label: command_ids::CMD_CREATE_UNION,
        menu_label: Some(command_ids::CMD_MENU_CREATE_UNION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "add_to_union",
        category: "edit",
        label: command_ids::CMD_ADD_TO_UNION,
        menu_label: Some(command_ids::CMD_MENU_ADD_TO_UNION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "remove_from_union",
        category: "edit",
        label: command_ids::CMD_REMOVE_FROM_UNION,
        menu_label: Some(command_ids::CMD_MENU_REMOVE_FROM_UNION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "break_union",
        category: "edit",
        label: command_ids::CMD_BREAK_UNION,
        menu_label: Some(command_ids::CMD_MENU_BREAK_UNION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
