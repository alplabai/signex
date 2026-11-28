//! Drawing commands: arcs, circles, ellipses, lines, rectangles, polygons
//! and curves on a schematic sheet, and graphics import.

use super::operands::{optional, required};
use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, Operand, OperandKind,
    operands,
};
use signex_i18n_ids::command_ids;

const fn operand(name: &'static str, kind: OperandKind, required: bool) -> Operand {
    Operand {
        name,
        kind,
        required,
    }
}

/// An ellipse (#1267): its centre and its two radii.
const ELLIPSE: &[Operand] = &[
    operand("center", OperandKind::Point, true),
    operand("radius_x", OperandKind::Length, true),
    operand("radius_y", OperandKind::Length, true),
    operand("document", OperandKind::Document, false),
];
/// An elliptical arc (#1267): the ellipse, then the start and end angles in
/// degrees, the arc running counter-clockwise from the start.
const ELLIPTICAL_ARC: &[Operand] = &[
    operand("center", OperandKind::Point, true),
    operand("radius_x", OperandKind::Length, true),
    operand("radius_y", OperandKind::Length, true),
    operand("start_angle", OperandKind::Turn, true),
    operand("end_angle", OperandKind::Turn, true),
    operand("document", OperandKind::Document, false),
];
/// A round rectangle (#1267): two opposite corners and the corner radius;
/// left out, 1.27 mm, held to half the short side.
const ROUND_RECTANGLE: &[Operand] = &[
    operand("start", OperandKind::Point, true),
    operand("end", OperandKind::Point, true),
    operand("corner_radius", OperandKind::Length, false),
    operand("document", OperandKind::Document, false),
];
/// A dimension (#2836): the two measured points, a point the dimension line
/// runs through (the tool's three clicks), then its Properties form:
/// `direction`, `units`, `precision`, `arrows` and the override `text`.
const DIMENSION: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    required("at", OperandKind::Point),
    optional("direction", OperandKind::Text),
    optional("units", OperandKind::Text),
    optional("precision", OperandKind::Text),
    optional("arrows", OperandKind::Text),
    optional("text", OperandKind::Text),
    optional("document", OperandKind::Document),
];

pub(super) const DRAWING: &[CommandMetadata] = &[
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
        flags: CommandFlags::MUTATES,
        operands: ELLIPTICAL_ARC,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_ellipse",
        category: "place",
        label: command_ids::CMD_DRAW_ELLIPSE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_ELLIPSE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: ELLIPSE,
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
        id: "draw_graphic_line",
        category: "place",
        label: command_ids::CMD_DRAW_GRAPHIC_LINE,
        menu_label: None,
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
        flags: CommandFlags::MUTATES,
        operands: ROUND_RECTANGLE,
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
    // #1300: a typed table, one click.
    CommandMetadata {
        id: "place_table",
        category: "place",
        label: command_ids::CMD_PLACE_TABLE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_TABLE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #1267: dimensions; Linear is horizontal or vertical by the drag.
    // #2836: the operands stand for the three clicks and the tool's form.
    CommandMetadata {
        id: "place_linear_dimension",
        category: "place",
        label: command_ids::CMD_PLACE_LINEAR_DIMENSION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_LINEAR_DIMENSION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: DIMENSION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_aligned_dimension",
        category: "place",
        label: command_ids::CMD_PLACE_ALIGNED_DIMENSION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_ALIGNED_DIMENSION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: DIMENSION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "import_graphics",
        category: "place",
        label: command_ids::CMD_IMPORT_GRAPHICS,
        menu_label: Some(command_ids::CMD_MENU_IMPORT_GRAPHICS),
        group: CommandGroup::General,
        enable: Enablement::RequiresDocument(super::DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::PLACE_FILE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_blanket",
        category: "place",
        label: command_ids::CMD_EDIT_BLANKET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::EDIT_BLANKET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_no_erc_rules",
        category: "place",
        label: command_ids::CMD_EDIT_NO_ERC_RULES,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::EDIT_NO_ERC_RULES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_arc",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_ARC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_ARC),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_string",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_STRING,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_STRING),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_dimension",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_DIMENSION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_DIMENSION),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2507: the other dimension kinds, in Place ▸ Dimension and the PCB
    // Active Bar's Dimensions dropdown. No default keys.
    CommandMetadata {
        id: "place_board_dimension_horizontal",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_DIMENSION_HORIZONTAL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_DIMENSION_HORIZONTAL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_dimension_vertical",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_DIMENSION_VERTICAL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_DIMENSION_VERTICAL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_dimension_radial",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_DIMENSION_RADIAL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_DIMENSION_RADIAL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2424: the Graphics tools, on the active non-copper layer. Each arms
    // its tool on the board in view; each drawing is one undo step. No
    // default keys.
    CommandMetadata {
        id: "place_board_line",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_LINE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_LINE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_graphic_arc",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_GRAPHIC_ARC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_GRAPHIC_ARC),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_circle",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_CIRCLE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_CIRCLE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_rectangle",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_RECTANGLE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_RECTANGLE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_text_frame",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_TEXT_FRAME,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_TEXT_FRAME),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2424: the Graphics tools and the drawing's Properties fields as an
    // agent or the CLI runs them, on the board `document` names; each is
    // one undo step and answers with the drawing as JSON.
    CommandMetadata {
        id: "add_board_graphic",
        category: "place",
        label: command_ids::CMD_ADD_BOARD_GRAPHIC,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_GRAPHIC_ADD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_board_graphic",
        category: "edit",
        label: command_ids::CMD_SET_BOARD_GRAPHIC,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_GRAPHIC_SET,
        ..CommandMetadata::DEFAULT
    },
    // #2368: a footprint's designator or comment placed on its own, as a
    // drag on the board moves it. No default key.
    CommandMetadata {
        id: "set_footprint_text",
        category: "edit",
        label: command_ids::CMD_SET_FOOTPRINT_TEXT,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::FOOTPRINT_TEXT_SET,
        ..CommandMetadata::DEFAULT
    },
    // #2410: DXF import. Without operands it asks for the file; the import
    // is one board undo step. No default key.
    CommandMetadata {
        id: "import_dxf",
        category: "file",
        label: command_ids::CMD_IMPORT_DXF,
        menu_label: Some(command_ids::CMD_MENU_IMPORT_DXF),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::IMPORT_DXF,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "import_dxf_outline",
        category: "file",
        label: command_ids::CMD_IMPORT_DXF_OUTLINE,
        menu_label: Some(command_ids::CMD_MENU_IMPORT_DXF_OUTLINE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::IMPORT_DXF_OUTLINE,
        ..CommandMetadata::DEFAULT
    },
];
