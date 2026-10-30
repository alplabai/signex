//! Parameter Manager table edits: cell writes, column add and remove.
//!
//! Every write goes through the mutation gateway, so each committed edit is
//! exactly one engine history entry and dirty-flagging stays correct.

use iced::Task;
use signex_engine::Command;
use uuid::Uuid;

use crate::app::Message;
use crate::app::state::{ModalId, Signex};

impl Signex {
    /// Write one parameter value now. An empty value empties the parameter,
    /// it does not delete it. Keystrokes in one cell are one undo step.
    pub(crate) fn handle_parameter_manager_edit(
        &mut self,
        symbol_uuid: Uuid,
        key: String,
        value: String,
    ) -> Task<Message> {
        // Same reasoning as `handle_move_selection_apply` (#585). The
        // gateway clears the content cache, syncs the canvas and refreshes
        // the panel context, so the hand-rolled versions of those are gone;
        // the overlay cache and the selection info were not touched before
        // and still are not.
        self.apply_engine_command(
            Command::SetSymbolField {
                symbol_id: symbol_uuid,
                key,
                value,
            },
            false,
            false,
        );
        Task::none()
    }

    /// Close the cell-typing run, so the next edit of the same cell is a
    /// new undo step.
    pub(crate) fn end_parameter_edit_run(&mut self) {
        if let Some(engine) = self.document_state.active_engine_mut() {
            engine.end_field_edit_run();
        }
    }

    /// Delete parameter `key` from every symbol that carries it, as one
    /// undo step.
    pub(crate) fn handle_parameter_removed(&mut self, key: &str) -> Task<Message> {
        let Some(engine) = self.document_state.active_engine() else {
            return Task::none();
        };
        let commands: Vec<Command> = engine
            .document()
            .symbols
            .iter()
            .filter(|s| s.fields.contains_key(key))
            .map(|s| Command::RemoveSymbolField {
                symbol_id: s.uuid,
                key: key.to_string(),
            })
            .collect();
        self.apply_engine_commands(commands, false, false);
        Task::none()
    }

    /// Add the typed name as a parameter on every non-power symbol, empty,
    /// as one undo step. A blank, reserved or already-present name adds
    /// nothing.
    pub(crate) fn handle_parameter_column_added(&mut self) -> Task<Message> {
        let key = self.ui_state.parameter_table.new_key.trim().to_string();
        if key.is_empty() || signex_engine::is_reserved_symbol_key(&key) {
            return Task::none();
        }
        let Some(engine) = self.document_state.active_engine() else {
            return Task::none();
        };
        let commands: Vec<Command> = engine
            .document()
            .symbols
            .iter()
            .filter(|s| !s.is_power && !s.fields.contains_key(&key))
            .map(|s| Command::SetSymbolField {
                symbol_id: s.uuid,
                key: key.clone(),
                value: String::new(),
            })
            .collect();
        if self.apply_engine_commands(commands, false, false) {
            self.ui_state.parameter_table.new_key.clear();
        }
        Task::none()
    }

    /// Close the manager's window and mark it closed.
    pub(crate) fn close_parameter_manager(&mut self) -> Task<Message> {
        self.end_parameter_edit_run();
        self.ui_state.parameter_manager_open = false;
        self.ui_state.parameter_table = Default::default();
        self.close_detached_modal(ModalId::ParameterManager)
    }
}
