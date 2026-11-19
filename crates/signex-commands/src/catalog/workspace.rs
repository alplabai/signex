//! Workspace commands: the command palette, preferences, shortcut help,
//! document tabs, window tiling and the panels.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, KeyRepeat};
use signex_i18n_ids::command_ids;

pub(super) const WORKSPACE: &[CommandMetadata] = &[
    CommandMetadata {
        id: "open_command_palette",
        category: "commands",
        label: command_ids::CMD_OPEN_COMMAND_PALETTE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_preferences",
        category: "preferences",
        label: command_ids::CMD_OPEN_PREFERENCES,
        menu_label: Some(command_ids::CMD_MENU_OPEN_PREFERENCES),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_schematic_preferences",
        category: "preferences",
        label: command_ids::CMD_OPEN_SCHEMATIC_PREFERENCES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_current_command_hotkeys",
        category: "help",
        label: command_ids::CMD_SHOW_CURRENT_COMMAND_HOTKEYS,
        menu_label: Some(command_ids::CMD_MENU_SHOW_CURRENT_COMMAND_HOTKEYS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_current_command_shortcuts",
        category: "help",
        label: command_ids::CMD_SHOW_CURRENT_COMMAND_SHORTCUTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_document_tab",
        category: "window",
        label: command_ids::CMD_NEXT_DOCUMENT_TAB,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_document_tab",
        category: "window",
        label: command_ids::CMD_PREVIOUS_DOCUMENT_TAB,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "tile_windows_horizontally",
        category: "window",
        label: command_ids::CMD_TILE_WINDOWS_HORIZONTALLY,
        menu_label: Some(command_ids::CMD_MENU_TILE_WINDOWS_HORIZONTALLY),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "tile_windows_vertically",
        category: "window",
        label: command_ids::CMD_TILE_WINDOWS_VERTICALLY,
        menu_label: Some(command_ids::CMD_MENU_TILE_WINDOWS_VERTICALLY),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "untile_documents",
        category: "window",
        label: command_ids::CMD_UNTILE_DOCUMENTS,
        menu_label: Some(command_ids::CMD_MENU_UNTILE_DOCUMENTS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_floating_panels",
        category: "window",
        label: command_ids::CMD_TOGGLE_FLOATING_PANELS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        key_repeat: KeyRepeat::Ignored,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_properties_panel",
        category: "panels",
        label: command_ids::CMD_TOGGLE_PROPERTIES_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        key_repeat: KeyRepeat::Ignored,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_schematic_filter_panel",
        category: "panels",
        label: command_ids::CMD_TOGGLE_SCHEMATIC_FILTER_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        key_repeat: KeyRepeat::Ignored,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_schematic_list_panel",
        category: "panels",
        label: command_ids::CMD_TOGGLE_SCHEMATIC_LIST_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        key_repeat: KeyRepeat::Ignored,
        ..CommandMetadata::DEFAULT
    },
];
