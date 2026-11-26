//! The board-and-schematic commands' operands (#2470, #2462, #2460): Update
//! PCB, New PCB, numbering, Back Annotate, the board against the schematic
//! and the board's status counters.

use super::{Operand, OperandKind, optional, required};

/// Update PCB without the dialog (#1312, #2470): for the project of
/// `document` (the project, one of its sheets or its board), `apply=true`
/// executes the rows ticked by default and `dry_run=true` lists them and
/// changes nothing. Either answers with the change list. Without `document`
/// it acts on the active project's board, so `apply=true` alone still runs;
/// with no board there, or no active project, it is refused, listing boards.
pub const UPDATE_PCB: &[Operand] = &[
    optional("document", OperandKind::Document),
    optional("apply", OperandKind::Bool),
    optional("dry_run", OperandKind::Bool),
];

/// New PCB without the dialog (#2232): `profile` is `project` (the default,
/// the project's own setup), a built-in token or a saved user profile.
/// `document` is the project (or a document of it) the board joins, else the
/// project in view; `name` is the board's file in the project folder (#2470).
pub const NEW_PCB: &[Operand] = &[
    optional("profile", OperandKind::Text),
    optional("document", OperandKind::Document),
    optional("name", OperandKind::Text),
];

/// Positional Re-Annotate of a board (#2470): `order` is `x_then_y` (the
/// default) or `y_then_x`, `bottom` is `after_top` (the default) or
/// `with_top`; `dry_run` lists the renames without making them.
pub const REANNOTATE_BOARD: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("order", OperandKind::Text),
    optional("bottom", OperandKind::Text),
    optional("dry_run", OperandKind::Bool),
];

/// Back Annotate of the project `document` belongs to (a project, its board
/// or a sheet), with the rows ticked by default (#2470); `dry_run` lists them.
pub const BACK_ANNOTATE: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("dry_run", OperandKind::Bool),
];

/// `check_board_against_schematic` (#2462): the board of the project the
/// named document is (or belongs to) against its schematic.
pub const CHECK_BOARD_AGAINST_SCHEMATIC: &[Operand] =
    &[required("document", OperandKind::Document)];

/// `apply_board_schematic_differences` (#2462): the difference ids (as the
/// check answered them, comma separated) to take to the board and to the
/// schematic; every other difference is skipped. `expect_revision` is the
/// board revision the check answered.
pub const APPLY_BOARD_SCHEMATIC_DIFFERENCES: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("to_board", OperandKind::Text),
    optional("to_schematic", OperandKind::Text),
    optional("expect_revision", OperandKind::Text),
];

/// `board_status` (#2460): the named board's status counters.
pub const BOARD_STATUS: &[Operand] = &[required("document", OperandKind::Document)];
