//! Transform commands: drag, move, rotate, flip and mirror the selection.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const TRANSFORM: &[CommandMetadata] = &[
    CommandMetadata {
        id: "drag",
        category: "modify",
        label: command_ids::CMD_DRAG,
        menu_label: Some(command_ids::CMD_MENU_DRAG),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "drag_selection",
        category: "modify",
        label: command_ids::CMD_DRAG_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_DRAG_SELECTION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "drag_keep_connections",
        category: "modify",
        label: command_ids::CMD_DRAG_KEEP_CONNECTIONS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_selection_xy",
        category: "modify",
        label: command_ids::CMD_MOVE_SELECTION_XY,
        menu_label: Some(command_ids::CMD_MENU_MOVE_SELECTION_XY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_selection",
        category: "modify",
        label: command_ids::CMD_MOVE_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_MOVE_SELECTION),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_object",
        category: "modify",
        label: command_ids::CMD_MOVE_OBJECT,
        menu_label: Some(command_ids::CMD_MENU_MOVE_OBJECT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_selection",
        category: "modify",
        label: command_ids::CMD_ROTATE_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_ROTATE_SELECTION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_selection_cw",
        category: "modify",
        label: command_ids::CMD_ROTATE_SELECTION_CW,
        menu_label: Some(command_ids::CMD_MENU_ROTATE_SELECTION_CW),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_clockwise",
        category: "modify",
        label: command_ids::CMD_ROTATE_CLOCKWISE,
        menu_label: Some(command_ids::CMD_MENU_ROTATE_CLOCKWISE),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_counterclockwise",
        category: "modify",
        label: command_ids::CMD_ROTATE_COUNTERCLOCKWISE,
        menu_label: Some(command_ids::CMD_MENU_ROTATE_COUNTERCLOCKWISE),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "flip_selected_x",
        category: "modify",
        label: command_ids::CMD_FLIP_SELECTED_X,
        menu_label: Some(command_ids::CMD_MENU_FLIP_SELECTED_X),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "flip_selected_y",
        category: "modify",
        label: command_ids::CMD_FLIP_SELECTED_Y,
        menu_label: Some(command_ids::CMD_MENU_FLIP_SELECTED_Y),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "mirror_x",
        category: "modify",
        label: command_ids::CMD_MIRROR_X,
        menu_label: Some(command_ids::CMD_MENU_MIRROR_X),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "mirror_y",
        category: "modify",
        label: command_ids::CMD_MIRROR_Y,
        menu_label: Some(command_ids::CMD_MENU_MIRROR_Y),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // Flip the selected footprints to the other side of the board (#2240).
    // No default key: a profile may bind one.
    CommandMetadata {
        id: "flip_selection",
        category: "modify",
        label: command_ids::CMD_FLIP_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_FLIP_SELECTION),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
