//! Search commands: find, find items headless, find and replace, step
//! through the matches, and the Search panel.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, KeyRepeat, operands};
use signex_i18n_ids::command_ids;

pub(super) const SEARCH: &[CommandMetadata] = &[
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
        id: "toggle_search_panel",
        category: "search",
        label: command_ids::CMD_TOGGLE_SEARCH_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        key_repeat: KeyRepeat::Ignored,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_similar_items",
        category: "search",
        label: command_ids::CMD_FIND_SIMILAR_ITEMS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::FIND_SIMILAR_ITEMS,
        ..CommandMetadata::DEFAULT
    },
];
