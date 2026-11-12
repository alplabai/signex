//! Command metadata for the schematic Active Bar's actions.
//!
//! Split out of `schematic.rs` rather than appended to it: the Active Bar
//! contributes more commands than every other schematic surface combined,
//! and `schematic.rs` would land near the 1000-line file cap. Entries are
//! `CommandGroup::Schematic` all the same — the file boundary is about
//! size, not grouping.
//!
//! The parameterised families are deliberately absent: `PlacePower*` (11),
//! `NetColor*` (8) and the four selection-arm modes each enumerate a value
//! domain and want one command plus an argument, which needs the
//! `CommandArgs` boundary (#367 / #366). Minting 23 frozen ids for them now
//! would be 23 names to deprecate later. See
//! `docs/audit/command-registry-action-surface-2026-07-25.md` §4.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n::command_ids;

pub(super) const ACTIVE_BAR: &[CommandMetadata] = &[
    CommandMetadata {
        id: "tool_select",
        category: "select",
        label: command_ids::CMD_TOOL_SELECT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "lasso_select",
        category: "select",
        label: command_ids::CMD_LASSO_SELECT,
        menu_label: Some(command_ids::CMD_MENU_LASSO_SELECT),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "select_connection",
        category: "select",
        label: command_ids::CMD_SELECT_CONNECTION,
        menu_label: Some(command_ids::CMD_MENU_SELECT_CONNECTION),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
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
        id: "place_sheet_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_SHEET_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SHEET_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SHEET_SYMBOL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_sheet_entry",
        category: "place",
        label: command_ids::CMD_PLACE_SHEET_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SHEET_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SHEET_ENTRY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_device_sheet_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_DEVICE_SHEET_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_DEVICE_SHEET_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_reuse_block",
        category: "place",
        label: command_ids::CMD_PLACE_REUSE_BLOCK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_REUSE_BLOCK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
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
        id: "draw_wire",
        category: "place",
        label: command_ids::CMD_DRAW_WIRE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_WIRE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::WIRE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_bus",
        category: "place",
        label: command_ids::CMD_DRAW_BUS,
        menu_label: Some(command_ids::CMD_MENU_DRAW_BUS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::BUS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_bus_entry",
        category: "place",
        label: command_ids::CMD_PLACE_BUS_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BUS_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_signal_harness",
        category: "place",
        label: command_ids::CMD_PLACE_SIGNAL_HARNESS,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SIGNAL_HARNESS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_connector",
        category: "place",
        label: command_ids::CMD_PLACE_HARNESS_CONNECTOR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HARNESS_CONNECTOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_entry",
        category: "place",
        label: command_ids::CMD_PLACE_HARNESS_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HARNESS_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_port",
        category: "place",
        label: command_ids::CMD_PLACE_PORT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_PORT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_off_sheet_connector",
        category: "place",
        label: command_ids::CMD_PLACE_OFF_SHEET_CONNECTOR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_OFF_SHEET_CONNECTOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_parameter_set",
        category: "place",
        label: command_ids::CMD_PLACE_PARAMETER_SET,
        menu_label: Some(command_ids::CMD_MENU_PLACE_PARAMETER_SET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_diff_pair",
        category: "place",
        label: command_ids::CMD_PLACE_DIFF_PAIR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_DIFF_PAIR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_blanket",
        category: "place",
        label: command_ids::CMD_PLACE_BLANKET,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BLANKET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_string",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT_STRING,
        menu_label: Some(command_ids::CMD_MENU_PLACE_TEXT_STRING),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_frame",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT_FRAME,
        menu_label: Some(command_ids::CMD_MENU_PLACE_TEXT_FRAME),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_note",
        category: "place",
        label: command_ids::CMD_PLACE_NOTE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NOTE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_arc",
        category: "place",
        label: command_ids::CMD_DRAW_ARC,
        menu_label: Some(command_ids::CMD_MENU_DRAW_ARC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_full_circle",
        category: "place",
        label: command_ids::CMD_DRAW_FULL_CIRCLE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_FULL_CIRCLE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_elliptical_arc",
        category: "place",
        label: command_ids::CMD_DRAW_ELLIPTICAL_ARC,
        menu_label: Some(command_ids::CMD_MENU_DRAW_ELLIPTICAL_ARC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_ellipse",
        category: "place",
        label: command_ids::CMD_DRAW_ELLIPSE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_ELLIPSE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_line",
        category: "place",
        label: command_ids::CMD_DRAW_LINE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_LINE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SEGMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_rectangle",
        category: "place",
        label: command_ids::CMD_DRAW_RECTANGLE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_RECTANGLE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SEGMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_round_rectangle",
        category: "place",
        label: command_ids::CMD_DRAW_ROUND_RECTANGLE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_ROUND_RECTANGLE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_polygon",
        category: "place",
        label: command_ids::CMD_DRAW_POLYGON,
        menu_label: Some(command_ids::CMD_MENU_DRAW_POLYGON),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_bezier",
        category: "place",
        label: command_ids::CMD_DRAW_BEZIER,
        menu_label: Some(command_ids::CMD_MENU_DRAW_BEZIER),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_graphic",
        category: "place",
        label: command_ids::CMD_PLACE_GRAPHIC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_GRAPHIC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_net_color",
        category: "view",
        label: command_ids::CMD_CLEAR_NET_COLOR,
        menu_label: Some(command_ids::CMD_MENU_CLEAR_NET_COLOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "color_net",
        category: "view",
        label: command_ids::CMD_COLOR_NET,
        menu_label: Some(command_ids::CMD_MENU_COLOR_NET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::COLOR_NET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_all_net_colors",
        category: "view",
        label: command_ids::CMD_CLEAR_ALL_NET_COLORS,
        menu_label: Some(command_ids::CMD_MENU_CLEAR_ALL_NET_COLORS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "assign_net_class",
        category: "design",
        label: command_ids::CMD_ASSIGN_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::ASSIGN_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_component",
        category: "place",
        label: command_ids::CMD_PLACE_COMPONENT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_COMPONENT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::PART,
        ..CommandMetadata::DEFAULT
    },
];
