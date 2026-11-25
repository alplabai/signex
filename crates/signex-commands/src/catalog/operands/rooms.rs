//! The rooms commands' operands (#2457, #2941): rooms made, placed and
//! filled from the command line. Board mm, as the file stores them.

use super::{Operand, OperandKind, optional, required};

/// `place_board_room`: the room's outline by two corners (`start`, `end`)
/// or by its `corners` `<x>,<y>;<x>,<y>;..`, and its `name` (else the next
/// `Room N`), the `sheet` placement it holds (`/<uuid>/..`) and its `side`
/// (`top`, `bottom` or `both`).
pub const PLACE_ROOM: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("start", OperandKind::Point),
    optional("end", OperandKind::Point),
    optional("corners", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("sheet", OperandKind::Text),
    optional("side", OperandKind::Text),
];
/// `create_rooms_from_sheets`: one room per sheet placement of the board's
/// project, with the dialog's options: `margin`, `shape` (`around_parts` or
/// `even_grid`), `side`, `move_parts`, `keep_parts_inside` and `replace`
/// (rooms of the same name), and the `rooms` to make by name, comma
/// separated (every one when left out).
pub const CREATE_ROOMS: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("margin", OperandKind::Length),
    optional("shape", OperandKind::Text),
    optional("side", OperandKind::Text),
    optional("move_parts", OperandKind::Bool),
    optional("keep_parts_inside", OperandKind::Bool),
    optional("replace", OperandKind::Bool),
    optional("rooms", OperandKind::Text),
];
/// `arrange_in_room`: the room by name or uuid.
pub const ARRANGE_ROOM: &[Operand] = &[
    required("document", OperandKind::Document),
    required("room", OperandKind::Text),
];
/// `auto_place`: the `rooms` to place by name or uuid, comma separated
/// (every room when left out), the priority `weights`
/// (`connection:3,thermal:1,separation:2,edges:5`), the gaps hot parts
/// (`thermal_gap`) and noisy and sensitive parts (`separation_gap`) keep,
/// and `dry_run`.
pub const AUTO_PLACE: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("rooms", OperandKind::Text),
    optional("weights", OperandKind::Text),
    optional("thermal_gap", OperandKind::Length),
    optional("separation_gap", OperandKind::Length),
    optional("dry_run", OperandKind::Bool),
];
