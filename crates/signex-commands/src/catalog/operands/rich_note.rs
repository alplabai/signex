//! A rich text note's operands (#1114, mockup row 203): its Frame and Size
//! sections, Apply of its editor (each with an optional `document`, as the
//! placements), and the editor's actions on a note of the sheet in view.

use super::{Operand, OperandKind, optional, required};

/// The Frame section of the note `id`: `frame` is `none`, `border` or
/// `filled`; `border` a width in mm or `auto`; `padding` in mm.
pub const TEXT_NOTE_FRAME: &[Operand] = &[
    required("id", OperandKind::Text),
    required("frame", OperandKind::Text),
    optional("border", OperandKind::Text),
    optional("padding", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// The Size section of the note `id`: `sizing` is `fit_text`,
/// `fixed_width` or `fixed_box`; `width` and `height` in mm.
pub const TEXT_NOTE_SIZE: &[Operand] = &[
    required("id", OperandKind::Text),
    required("sizing", OperandKind::Text),
    optional("width", OperandKind::Text),
    optional("height", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// Apply to the note `id`: its text becomes `source`, or the open editor's.
pub const RICH_NOTE_APPLY: &[Operand] = &[
    required("id", OperandKind::Text),
    optional("source", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// Open the editor on the note `id`, the cursor at 1-based `line` and
/// `column`.
pub const RICH_NOTE_EDIT: &[Operand] = &[
    required("id", OperandKind::Text),
    optional("line", OperandKind::Text),
    optional("column", OperandKind::Text),
];
/// The note `id` alone: Cancel, and the compile.
pub const RICH_NOTE: &[Operand] = &[required("id", OperandKind::Text)];
/// A toolbar button on the open editor of the note `id`: `style` is `bold`,
/// `italic`, `heading`, `list`, `math`, `link` or `table`.
pub const RICH_NOTE_WRAP: &[Operand] = &[
    required("id", OperandKind::Text),
    required("style", OperandKind::Text),
];
/// A link of the note `id`, as its source writes it.
pub const RICH_NOTE_LINK: &[Operand] = &[
    required("id", OperandKind::Text),
    required("target", OperandKind::Text),
];
