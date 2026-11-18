//! Arrange commands: the stacking order of objects and drawings, and the
//! align and distribute steps.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const ARRANGE: &[CommandMetadata] = &[
    CommandMetadata {
        id: "bring_drawings_forward",
        category: "edit",
        label: command_ids::CMD_BRING_DRAWINGS_FORWARD,
        menu_label: Some(command_ids::CMD_MENU_BRING_DRAWINGS_FORWARD),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bring_drawings_to_front",
        category: "edit",
        label: command_ids::CMD_BRING_DRAWINGS_TO_FRONT,
        menu_label: Some(command_ids::CMD_MENU_BRING_DRAWINGS_TO_FRONT),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_drawings_backward",
        category: "edit",
        label: command_ids::CMD_SEND_DRAWINGS_BACKWARD,
        menu_label: Some(command_ids::CMD_MENU_SEND_DRAWINGS_BACKWARD),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_drawings_to_back",
        category: "edit",
        label: command_ids::CMD_SEND_DRAWINGS_TO_BACK,
        menu_label: Some(command_ids::CMD_MENU_SEND_DRAWINGS_TO_BACK),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_to_front",
        category: "modify",
        label: command_ids::CMD_MOVE_TO_FRONT,
        menu_label: Some(command_ids::CMD_MENU_MOVE_TO_FRONT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bring_to_front",
        category: "modify",
        label: command_ids::CMD_BRING_TO_FRONT,
        menu_label: Some(command_ids::CMD_MENU_BRING_TO_FRONT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_to_back",
        category: "modify",
        label: command_ids::CMD_SEND_TO_BACK,
        menu_label: Some(command_ids::CMD_MENU_SEND_TO_BACK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bring_to_front_of",
        category: "modify",
        label: command_ids::CMD_BRING_TO_FRONT_OF,
        menu_label: Some(command_ids::CMD_MENU_BRING_TO_FRONT_OF),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_to_back_of",
        category: "modify",
        label: command_ids::CMD_SEND_TO_BACK_OF,
        menu_label: Some(command_ids::CMD_MENU_SEND_TO_BACK_OF),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_left",
        category: "modify",
        label: command_ids::CMD_ALIGN_LEFT,
        menu_label: Some(command_ids::CMD_MENU_ALIGN_LEFT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_right",
        category: "modify",
        label: command_ids::CMD_ALIGN_RIGHT,
        menu_label: Some(command_ids::CMD_MENU_ALIGN_RIGHT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_horizontal_centers",
        category: "modify",
        label: command_ids::CMD_ALIGN_HORIZONTAL_CENTERS,
        menu_label: Some(command_ids::CMD_MENU_ALIGN_HORIZONTAL_CENTERS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "distribute_horizontally",
        category: "modify",
        label: command_ids::CMD_DISTRIBUTE_HORIZONTALLY,
        menu_label: Some(command_ids::CMD_MENU_DISTRIBUTE_HORIZONTALLY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_top",
        category: "modify",
        label: command_ids::CMD_ALIGN_TOP,
        menu_label: Some(command_ids::CMD_MENU_ALIGN_TOP),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_bottom",
        category: "modify",
        label: command_ids::CMD_ALIGN_BOTTOM,
        menu_label: Some(command_ids::CMD_MENU_ALIGN_BOTTOM),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_vertical_centers",
        category: "modify",
        label: command_ids::CMD_ALIGN_VERTICAL_CENTERS,
        menu_label: Some(command_ids::CMD_MENU_ALIGN_VERTICAL_CENTERS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "distribute_vertically",
        category: "modify",
        label: command_ids::CMD_DISTRIBUTE_VERTICALLY,
        menu_label: Some(command_ids::CMD_MENU_DISTRIBUTE_VERTICALLY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_to_grid",
        category: "modify",
        label: command_ids::CMD_ALIGN_TO_GRID,
        menu_label: Some(command_ids::CMD_MENU_ALIGN_TO_GRID),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "arrange_in_box",
        category: "modify",
        label: command_ids::CMD_ARRANGE_IN_BOX,
        menu_label: Some(command_ids::CMD_MENU_ARRANGE_IN_BOX),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::ARRANGE_BOX,
        ..CommandMetadata::DEFAULT
    },
];
