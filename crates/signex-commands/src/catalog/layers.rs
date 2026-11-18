//! Board layer commands: step through the layers, single-layer mode, layer
//! visibility, the Layers panel and the ratsnest.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const LAYERS: &[CommandMetadata] = &[
    CommandMetadata {
        id: "next_layer",
        category: "view",
        label: command_ids::CMD_NEXT_LAYER,
        menu_label: Some(command_ids::CMD_MENU_NEXT_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "prev_layer",
        category: "view",
        label: command_ids::CMD_PREV_LAYER,
        menu_label: Some(command_ids::CMD_MENU_PREV_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2357: `*` steps through the signal layers only, `+` through every
    // layer the board lists.
    CommandMetadata {
        id: "next_signal_layer",
        category: "view",
        label: command_ids::CMD_NEXT_SIGNAL_LAYER,
        menu_label: Some(command_ids::CMD_MENU_NEXT_SIGNAL_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_single_layer_mode",
        category: "view",
        label: command_ids::CMD_TOGGLE_SINGLE_LAYER_MODE,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_SINGLE_LAYER_MODE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_layer_visibility",
        category: "view",
        label: command_ids::CMD_TOGGLE_LAYER_VISIBILITY,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_LAYER_VISIBILITY),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_all_layers",
        category: "view",
        label: command_ids::CMD_SHOW_ALL_LAYERS,
        menu_label: Some(command_ids::CMD_MENU_SHOW_ALL_LAYERS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_layers_panel",
        category: "view",
        label: command_ids::CMD_TOGGLE_LAYERS_PANEL,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_LAYERS_PANEL),
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_ratsnest",
        category: "view",
        label: command_ids::CMD_TOGGLE_RATSNEST,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_RATSNEST),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2394: the layer strip under the board. View state and per-user
    // settings only: no undo step, no dirty mark. The rows with operands run
    // from an agent or the CLI on the board `document` names (the per-user
    // ones name none); a person uses the tab menu or the layer set button,
    // so without operands they say so and do nothing.
    CommandMetadata {
        id: "show_layer_set",
        category: "view",
        label: command_ids::CMD_SHOW_LAYER_SET,
        menu_label: Some(command_ids::CMD_MENU_SHOW_LAYER_SET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_layer_set",
        category: "view",
        label: command_ids::CMD_SAVE_LAYER_SET,
        menu_label: Some(command_ids::CMD_MENU_SAVE_LAYER_SET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "delete_layer_set",
        category: "view",
        label: command_ids::CMD_DELETE_LAYER_SET,
        menu_label: Some(command_ids::CMD_MENU_DELETE_LAYER_SET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_SET_NAME,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "hide_layer",
        category: "view",
        label: command_ids::CMD_HIDE_LAYER,
        menu_label: Some(command_ids::CMD_MENU_HIDE_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_layer",
        category: "view",
        label: command_ids::CMD_HIGHLIGHT_LAYER,
        menu_label: Some(command_ids::CMD_MENU_HIGHLIGHT_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "hide_layer_group",
        category: "view",
        label: command_ids::CMD_HIDE_LAYER_GROUP,
        menu_label: Some(command_ids::CMD_MENU_HIDE_LAYER_GROUP),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_GROUP,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_layer_group",
        category: "view",
        label: command_ids::CMD_SHOW_LAYER_GROUP,
        menu_label: Some(command_ids::CMD_MENU_SHOW_LAYER_GROUP),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_GROUP,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_layer_tab_names",
        category: "view",
        label: command_ids::CMD_SET_LAYER_TAB_NAMES,
        menu_label: Some(command_ids::CMD_MENU_SET_LAYER_TAB_NAMES),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_TAB_NAMES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_layer_numbers",
        category: "view",
        label: command_ids::CMD_TOGGLE_LAYER_NUMBERS,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_LAYER_NUMBERS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2401: open the board's layer stack manager (the Layers tab menu's
    // "Layer stack..." row runs it too). No default key.
    CommandMetadata {
        id: "board_layer_stack",
        category: "design",
        label: command_ids::CMD_BOARD_LAYER_STACK,
        menu_label: Some(command_ids::CMD_MENU_BOARD_LAYER_STACK),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2401: the manager's edit for an agent or the CLI: the whole stack
    // as JSON rows, one undo step, answered with the stack it stored.
    CommandMetadata {
        id: "set_layer_stack",
        category: "design",
        label: command_ids::CMD_SET_LAYER_STACK,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_LAYER_STACK,
        ..CommandMetadata::DEFAULT
    },
    // #2401: per-layer copper sizes of vias or pads; the Via and Pad
    // Stack rows are its surface. One undo step.
    CommandMetadata {
        id: "set_layer_sizes",
        category: "modify",
        label: command_ids::CMD_SET_LAYER_SIZES,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_LAYER_SIZES,
        ..CommandMetadata::DEFAULT
    },
    // #2242: a pad's copper shape with its trapezoid delta or custom
    // outline, the Pad panel's Shape row in agent form. One undo step.
    CommandMetadata {
        id: "set_pad_shape",
        category: "modify",
        label: command_ids::CMD_SET_PAD_SHAPE,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_PAD_SHAPE,
        ..CommandMetadata::DEFAULT
    },
    // #2363: the Layers panel's board view configuration, on the named
    // board's view: never the board, no undo step.
    CommandMetadata {
        id: "set_layer_visible",
        category: "view",
        label: command_ids::CMD_SET_LAYER_VISIBLE,
        menu_label: Some(command_ids::CMD_MENU_SET_LAYER_VISIBLE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_VISIBLE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_active_layer",
        category: "view",
        label: command_ids::CMD_SET_ACTIVE_LAYER,
        menu_label: Some(command_ids::CMD_MENU_SET_ACTIVE_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_layer_color",
        category: "view",
        label: command_ids::CMD_SET_LAYER_COLOR,
        menu_label: Some(command_ids::CMD_MENU_SET_LAYER_COLOR),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_COLOR,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_used_layers_only",
        category: "view",
        label: command_ids::CMD_SHOW_USED_LAYERS_ONLY,
        menu_label: Some(command_ids::CMD_MENU_SHOW_USED_LAYERS_ONLY),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::ON_BOARD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_object_visible",
        category: "view",
        label: command_ids::CMD_SET_OBJECT_VISIBLE,
        menu_label: Some(command_ids::CMD_MENU_SET_OBJECT_VISIBLE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::OBJECT_VISIBLE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_object_transparency",
        category: "view",
        label: command_ids::CMD_SET_OBJECT_TRANSPARENCY,
        menu_label: Some(command_ids::CMD_MENU_SET_OBJECT_TRANSPARENCY),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::OBJECT_TRANSPARENCY,
        ..CommandMetadata::DEFAULT
    },
];
