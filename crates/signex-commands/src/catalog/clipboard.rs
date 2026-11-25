//! Clipboard commands: copy, cut, the paste variants, duplicate, delete
//! and rubber-stamp copies of the selection.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const CLIPBOARD: &[CommandMetadata] = &[
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
        id: "delete_selection",
        category: "edit",
        label: command_ids::CMD_DELETE_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_DELETE_SELECTION),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        operands: super::operands::DOCUMENT_ITEMS,
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
];
