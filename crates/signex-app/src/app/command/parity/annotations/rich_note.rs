//! The rich text note's annotations (#1114). The editor and the compile act
//! on a note of the sheet in view; Cancel and a toolbar button read the
//! editor's typing. Apply from operands takes `source` and reads nothing
//! implicit (a call without it is refused, Caner 2026-11-27).

use super::super::forms::Form::Operand;
use super::super::inputs::Input::{ActiveDocument, InProgressEdit};
use super::Annotation;

pub(super) const RICH_NOTE: &[Annotation] = &[
    (
        &["edit_rich_note", "compile_rich_note", "open_rich_note_link"],
        Operand,
        &[ActiveDocument],
    ),
    (
        &["cancel_rich_note_edit", "wrap_rich_note_selection"],
        Operand,
        &[ActiveDocument, InProgressEdit],
    ),
    (&["apply_rich_note_edit"], Operand, &[]),
];
