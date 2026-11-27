//! Transform commands: drag, move, rotate, flip and mirror the selection.

use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, footprint_operands,
    operands,
};
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
    // #2418: the Properties rows of a placed object. Each runs from its
    // operands only; the Properties row sends the same edit.
    CommandMetadata {
        id: "set_object_position",
        category: "edit",
        label: command_ids::CMD_SET_OBJECT_POSITION,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::OBJECT_POSITION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_object_rotation",
        category: "edit",
        label: command_ids::CMD_SET_OBJECT_ROTATION,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::OBJECT_ROTATION,
        ..CommandMetadata::DEFAULT
    },
    // #2936: a board footprint moved, turned or flipped by id on the board
    // `document` names, one undo step each. Each runs from its operands
    // only; a person drags the footprint or uses the Component view.
    CommandMetadata {
        id: "set_footprint_position",
        category: "edit",
        label: command_ids::CMD_SET_FOOTPRINT_POSITION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: footprint_operands::FOOTPRINT_POSITION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_footprint_rotation",
        category: "edit",
        label: command_ids::CMD_SET_FOOTPRINT_ROTATION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: footprint_operands::FOOTPRINT_ROTATION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_footprint_side",
        category: "edit",
        label: command_ids::CMD_SET_FOOTPRINT_SIDE,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: footprint_operands::FOOTPRINT_SIDE,
        ..CommandMetadata::DEFAULT
    },
];
