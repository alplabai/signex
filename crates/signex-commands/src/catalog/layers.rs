//! Board layer commands: step through the layers, single-layer mode, layer
//! visibility, the Layers panel and the ratsnest.

use super::operands::{optional, required};
use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, Operand, OperandKind,
};
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
    // #2578: a pour's Corners and Corner Size rows in agent form. One undo
    // step.
    CommandMetadata {
        id: "set_pour_corners",
        category: "modify",
        label: command_ids::CMD_SET_POUR_CORNERS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: SET_POUR_CORNERS,
        ..CommandMetadata::DEFAULT
    },
    // #2579: a pour's Fill Mode and hatch rows in agent form. One undo step.
    CommandMetadata {
        id: "set_pour_fill",
        category: "modify",
        label: command_ids::CMD_SET_POUR_FILL,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: SET_POUR_FILL,
        ..CommandMetadata::DEFAULT
    },
    // #2580: a pour's Thermal Relief switch, spoke width and air gap, the
    // pour panel's rows in agent form. One undo step.
    CommandMetadata {
        id: "set_pour_thermal_relief",
        category: "modify",
        label: command_ids::CMD_SET_POUR_THERMAL_RELIEF,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_POUR_THERMAL_RELIEF,
        ..CommandMetadata::DEFAULT
    },
    // #2593: a footprint pad's own thermal relief air gap, spoke width and
    // spoke count, each Rule or Manual. One undo step.
    CommandMetadata {
        id: "set_pad_thermal_relief",
        category: "modify",
        label: command_ids::CMD_SET_PAD_THERMAL_RELIEF,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_PAD_THERMAL_RELIEF,
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
        // Layer colours are saved with the board (2026-11-18).
        flags: CommandFlags::MUTATES,
        operands: super::operands::LAYER_COLOR,
        ..CommandMetadata::DEFAULT
    },
    // 2026-11-18: Layer colour... on a layer tab; the palette opens it for
    // the active layer. It opens the Layers panel's colour picker.
    CommandMetadata {
        id: "edit_layer_color",
        category: "view",
        label: command_ids::CMD_EDIT_LAYER_COLOR,
        menu_label: Some(command_ids::CMD_MENU_EDIT_LAYER_COLOR),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // 2026-11-18: Reset all layer colours on a layer tab, one undo step.
    CommandMetadata {
        id: "reset_layer_colors",
        category: "view",
        label: command_ids::CMD_RESET_LAYER_COLORS,
        menu_label: Some(command_ids::CMD_MENU_RESET_LAYER_COLORS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        // A CLI or an agent names the board.
        flags: CommandFlags::MUTATES,
        operands: super::operands::DOCUMENT,
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

/// `set_pour_corners` (#2578): pours by uuid, their `corners` (`sharp`,
/// `rounded` or `chamfer`) and, optionally, the `corner_size` in mm (a
/// Rounded corner's radius, a Chamfer's cut; 0 or more).
const SET_POUR_CORNERS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    required("corners", OperandKind::Text),
    optional("corner_size", OperandKind::Text),
];

/// `set_pour_fill` (#2579): pours by uuid, their `fill` (`solid` or
/// `hatched`) and, optionally, the hatch line `width` and the `gap` between
/// lines in mm (0 or more; 0 uses the default).
const SET_POUR_FILL: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    required("fill", OperandKind::Text),
    optional("width", OperandKind::Text),
    optional("gap", OperandKind::Text),
];
