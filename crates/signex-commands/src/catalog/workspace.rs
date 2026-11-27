//! Workspace commands: the command palette, preferences, shortcut help,
//! document tabs, window tiling and the panels.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement};
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
    // #2495: a key binding in a custom keyboard profile, from a CLI run or
    // a script; a person uses Preferences ▸ Keyboard shortcuts.
    CommandMetadata {
        id: "keymap_assign",
        category: "preferences",
        label: command_ids::CMD_KEYMAP_ASSIGN,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::KEYMAP_ASSIGN,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "keymap_clear",
        category: "preferences",
        label: command_ids::CMD_KEYMAP_CLEAR,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::KEYMAP_CLEAR,
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
    // Mockup row 207: up to four panes, one document kind each.
    CommandMetadata {
        id: "tile_windows_grid",
        category: "window",
        label: command_ids::CMD_TILE_WINDOWS_GRID,
        menu_label: Some(command_ids::CMD_MENU_TILE_WINDOWS_GRID),
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
    // Mockup row 207 (#1307): the document area's panes.
    CommandMetadata {
        id: "split_pane_right",
        category: "window",
        label: command_ids::CMD_SPLIT_PANE_RIGHT,
        menu_label: Some(command_ids::CMD_MENU_SPLIT_PANE_RIGHT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "split_pane_down",
        category: "window",
        label: command_ids::CMD_SPLIT_PANE_DOWN,
        menu_label: Some(command_ids::CMD_MENU_SPLIT_PANE_DOWN),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_second_view",
        category: "window",
        label: command_ids::CMD_OPEN_SECOND_VIEW,
        menu_label: Some(command_ids::CMD_MENU_OPEN_SECOND_VIEW),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // Mockup row 207: the pane menu.
    CommandMetadata {
        id: "move_tab_to_other_pane",
        category: "window",
        label: command_ids::CMD_MOVE_TAB_TO_OTHER_PANE,
        menu_label: Some(command_ids::CMD_MENU_MOVE_TAB_TO_OTHER_PANE),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "close_pane",
        category: "window",
        label: command_ids::CMD_CLOSE_PANE,
        menu_label: Some(command_ids::CMD_MENU_CLOSE_PANE),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "sync_pane_zoom",
        category: "window",
        label: command_ids::CMD_SYNC_PANE_ZOOM,
        menu_label: Some(command_ids::CMD_MENU_SYNC_PANE_ZOOM),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "equalize_panes",
        category: "window",
        label: command_ids::CMD_EQUALIZE_PANES,
        menu_label: Some(command_ids::CMD_MENU_EQUALIZE_PANES),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2470: a dock panel opened, closed, collapsed or expanded by its
    // layout key, for an agent or a script. No menu row and no default key:
    // a person uses the panel list, the tab and the region's arrow.
    CommandMetadata {
        id: "set_dock_panel",
        category: "panels",
        label: command_ids::CMD_SET_DOCK_PANEL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::DOCK_PANEL,
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
        ..CommandMetadata::DEFAULT
    },
    // Workspace layouts (mockup row 166): View ▸ Window ▸ Layouts. Unbound by default:
    // docs/WORKSPACE_LAYOUTS.md, "Shortcuts".
    CommandMetadata {
        id: "apply_layout_schematic",
        category: "window",
        label: command_ids::CMD_APPLY_LAYOUT_SCHEMATIC,
        menu_label: Some(command_ids::CMD_MENU_APPLY_LAYOUT_SCHEMATIC),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "apply_layout_board",
        category: "window",
        label: command_ids::CMD_APPLY_LAYOUT_BOARD,
        menu_label: Some(command_ids::CMD_MENU_APPLY_LAYOUT_BOARD),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "apply_layout_review",
        category: "window",
        label: command_ids::CMD_APPLY_LAYOUT_REVIEW,
        menu_label: Some(command_ids::CMD_MENU_APPLY_LAYOUT_REVIEW),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "apply_layout_notebook",
        category: "window",
        label: command_ids::CMD_APPLY_LAYOUT_NOTEBOOK,
        menu_label: Some(command_ids::CMD_MENU_APPLY_LAYOUT_NOTEBOOK),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_workspace_layout",
        category: "window",
        label: command_ids::CMD_SAVE_WORKSPACE_LAYOUT,
        menu_label: Some(command_ids::CMD_MENU_SAVE_WORKSPACE_LAYOUT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "manage_workspace_layouts",
        category: "window",
        label: command_ids::CMD_MANAGE_WORKSPACE_LAYOUTS,
        menu_label: Some(command_ids::CMD_MENU_MANAGE_WORKSPACE_LAYOUTS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "reset_workspace_layout",
        category: "window",
        label: command_ids::CMD_RESET_WORKSPACE_LAYOUT,
        menu_label: Some(command_ids::CMD_MENU_RESET_WORKSPACE_LAYOUT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
