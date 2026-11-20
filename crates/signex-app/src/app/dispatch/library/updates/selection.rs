//! The Library Updates modal's ticks and conflict answers: state edits only,
//! no IO.

use iced::Task;

use crate::app::{Message, Signex};

impl Signex {
    /// Toggle one row's checkbox in the Library Updates modal.
    pub(in crate::app::dispatch::library) fn handle_library_updates_toggle_selection(
        &mut self,
        symbol_uuid: uuid::Uuid,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_updates.as_mut() {
            state.toggle(symbol_uuid);
        }
        Task::none()
    }

    /// The header checkbox of the Library Updates table: clear every row
    /// when all are ticked, else tick every row.
    pub(in crate::app::dispatch::library) fn handle_library_updates_select_all_toggled(
        &mut self,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_updates.as_mut() {
            state.toggle_all();
        }
        Task::none()
    }

    /// The user answered one conflict of a part in the Library Updates modal.
    pub(in crate::app::dispatch::library) fn handle_library_updates_conflict_answered(
        &mut self,
        symbol_uuid: uuid::Uuid,
        field: signex_types::schematic::StampField,
        choice: crate::library::part_sync::Choice,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_updates.as_mut() {
            state.answer_conflict(symbol_uuid, field, choice);
        }
        Task::none()
    }
}
