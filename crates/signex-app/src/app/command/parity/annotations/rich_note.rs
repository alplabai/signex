//! The rich text note's annotations (#1114). The editor and the compile act
//! on a note of the sheet in view; Cancel, a toolbar button and Apply
//! without `source` read the editor's typing.

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
    (&["apply_rich_note_edit"], Operand, &[InProgressEdit]),
];
