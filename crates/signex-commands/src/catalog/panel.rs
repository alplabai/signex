//! Panelisation (#2410): Tools ▸ Panelize Board and Remove Panel, the
//! `set_board_panel` edit the Properties panel's Panel section makes, and the
//! `board_panel` read. In a window they act on the board in view; named a
//! board (`document`), on that board. Each edit is one undo step.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const PANEL: &[CommandMetadata] = &[
    CommandMetadata {
        id: "panelize_board",
        category: "pcb",
        label: command_ids::CMD_PANELIZE_BOARD,
        menu_label: Some(command_ids::CMD_MENU_PANELIZE_BOARD),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "remove_board_panel",
        category: "pcb",
        label: command_ids::CMD_REMOVE_BOARD_PANEL,
        menu_label: Some(command_ids::CMD_MENU_REMOVE_BOARD_PANEL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_board_panel",
        category: "pcb",
        label: command_ids::CMD_SET_BOARD_PANEL,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::SET_BOARD_PANEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_panel",
        category: "pcb",
        label: command_ids::CMD_BOARD_PANEL,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
];
