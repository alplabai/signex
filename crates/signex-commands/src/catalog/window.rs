//! Window ▸ Tile command metadata (#2247, #2249): the documents in panes of
//! the main window and the undocked document windows, arranged side by side
//! or stacked, and the way back to one document at a time.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const WINDOW: &[CommandMetadata] = &[
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
];
