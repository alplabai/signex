//! Drawing commands: arcs, circles, ellipses, lines, rectangles, polygons
//! and curves on a schematic sheet, and graphics import.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

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
        id: "import_graphics",
        category: "library",
        label: command_ids::CMD_IMPORT_GRAPHICS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
