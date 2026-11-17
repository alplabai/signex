//! History commands: undo, redo and repeat the last placed item.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const HISTORY: &[CommandMetadata] = &[
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
        id: "repeat_last_item",
        category: "edit",
        label: command_ids::CMD_REPEAT_LAST_ITEM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
