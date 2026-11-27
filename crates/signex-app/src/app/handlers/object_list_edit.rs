//! The Object List's cell editor (#2953): open a cell with the text it
//! stores, keep the draft, and on Enter write it through
//! `Signex::set_object_text`, the edit the Properties rows and
//! `set_object_text` send, as one undo step. A cell that cannot be edited
//! here (another sheet than the one in view, or a cell with no single text)
//! zooms to its object, as a double click on the row does.

use iced::Task;

use super::super::*;
use crate::panels::object_list::cell_edit::{
    CELL_INPUT_ID, CellEditMsg, CellEditing, ObjectColumn, editable_text,
};
use crate::panels::{ObjectListMsg, ObjectRef, PanelMsg};

impl Signex {
    pub(crate) fn handle_object_list_cell_edit(&mut self, msg: CellEditMsg) -> Task<Message> {
        match msg {
            CellEditMsg::Opened { target, column } => self.open_object_list_cell(target, column),
            CellEditMsg::DraftChanged(text) => {
                if let Some(editing) = self.ui_state.object_list.editing.as_mut() {
                    editing.draft = text;
                }
                Task::none()
            }
            CellEditMsg::Committed => {
                self.commit_object_list_cell();
                Task::none()
            }
            CellEditMsg::Cancelled => {
                self.ui_state.object_list.editing = None;
                Task::none()
            }
        }
    }

    /// Esc while a cell is open puts its text back. `false` when none is.
    pub(crate) fn cancel_object_list_cell_edit(&mut self) -> bool {
        self.ui_state.object_list.editing.take().is_some()
    }

    /// Open the `column` cell of the object `target` with its stored text,
    /// and focus it.
    fn open_object_list_cell(&mut self, target: ObjectRef, column: ObjectColumn) -> Task<Message> {
        let on_sheet = self.document_state.active_path.as_deref() == Some(target.sheet.as_path());
        let text = self
            .ui_state
            .object_list
            .source
            .rows
            .iter()
            .find(|row| row.target == target)
            .and_then(|row| editable_text(row, column))
            .filter(|_| on_sheet);
        let original = text.and_then(|text| self.stored_object_text(text));
        let (Some(text), Some(original)) = (text, original) else {
            return Task::done(Message::Dock(crate::dock::DockMessage::Panel(
                PanelMsg::ObjectList(ObjectListMsg::RowDoubleClicked(target)),
            )));
        };
        self.ui_state.object_list.editing = Some(CellEditing {
            target,
            column,
            text,
            draft: original.clone(),
            original,
        });
        iced::widget::operation::focus(iced::widget::Id::new(CELL_INPUT_ID))
    }

    /// Enter: write the draft as one edit and close the cell. A draft equal
    /// to the stored text writes nothing; a refusal says why in Messages.
    fn commit_object_list_cell(&mut self) {
        let Some(editing) = self.ui_state.object_list.editing.take() else {
            return;
        };
        if editing.draft == editing.original {
            return;
        }
        let on_sheet =
            self.document_state.active_path.as_deref() == Some(editing.target.sheet.as_path());
        if !on_sheet {
            crate::diagnostics::log_warning(format!(
                "Object List: the text typed into {} was not applied, its sheet is no longer \
                 in view. The text was: {}",
                editing.target.sheet.display(),
                editing.draft
            ));
            return;
        }
        if let Err(refusal) =
            self.set_object_text(editing.target.item, editing.text, &editing.draft)
            && !refusal.logged
        {
            crate::diagnostics::log_warning(format!("Object List: {}", refusal.reason));
        }
    }
}

#[cfg(test)]
#[path = "object_list_edit_tests.rs"]
mod tests;
