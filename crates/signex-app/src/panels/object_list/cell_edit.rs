//! Editing a cell of the Object List in place (#2953, decided by the
//! maintainer 2026-11-25): a double click on an editable Name or Value cell
//! opens it with the stored text, Enter commits it as one undo step through
//! the edit Properties sends for that field (`Signex::set_object_text`), Esc
//! puts the text back. Which cells are editable is decided here, once.

use signex_engine::TextTarget;
use signex_types::schematic::SelectedKind;

use super::model::{ObjectKind, ObjectRef, ObjectRow};

/// The id of the cell editor's text box, to focus it when it opens.
pub const CELL_INPUT_ID: &str = "object-list-cell";

/// The two columns a cell can be edited in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectColumn {
    Name,
    Value,
}

/// The stored text the cell of `row` in `column` edits; `None` for a cell
/// that is not editable here. A part's designator and value, a power port's
/// name (its value, or a power label's text), a label's or port's name and a
/// text note's text are; a sheet symbol, a sheet entry, a wire's length and
/// the nouns are not (their edits are not one text).
pub fn editable_text(row: &ObjectRow, column: ObjectColumn) -> Option<TextTarget> {
    let id = row.target.item.uuid;
    let symbol = row.target.item.kind == SelectedKind::Symbol;
    match (row.kind, column) {
        (ObjectKind::Part, ObjectColumn::Name) => Some(TextTarget::SymbolReference(id)),
        (ObjectKind::Part, ObjectColumn::Value) => Some(TextTarget::SymbolValue(id)),
        (ObjectKind::PowerPort, ObjectColumn::Name) if symbol => Some(TextTarget::SymbolValue(id)),
        (ObjectKind::PowerPort | ObjectKind::Label | ObjectKind::Port, ObjectColumn::Name) => {
            Some(TextTarget::Label(id))
        }
        (ObjectKind::Text, ObjectColumn::Value) => Some(TextTarget::TextNote(id)),
        _ => None,
    }
}

/// An open cell editor: which object and field, and the text typed so far.
#[derive(Debug, Clone, PartialEq)]
pub struct CellEditing {
    pub target: ObjectRef,
    pub column: ObjectColumn,
    pub text: TextTarget,
    /// The stored text when the cell opened; Enter with it unchanged edits
    /// nothing.
    pub original: String,
    pub draft: String,
}

impl CellEditing {
    /// Whether this editor is the cell of `row` in `column`.
    pub fn is_cell(&self, row: &ObjectRow, column: ObjectColumn) -> bool {
        self.column == column && self.target == row.target
    }
}

/// What happened in a cell editor.
#[derive(Debug, Clone)]
pub enum CellEditMsg {
    /// An editable cell was double-clicked: open it.
    Opened {
        target: ObjectRef,
        column: ObjectColumn,
    },
    DraftChanged(String),
    /// Enter: write the draft.
    Committed,
    /// Esc: put the text back.
    Cancelled,
}

#[cfg(test)]
#[path = "cell_edit_tests.rs"]
mod tests;
