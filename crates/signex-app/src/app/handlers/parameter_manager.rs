//! Parameter Manager table edits: cell drafts, column add and remove.
//!
//! Every write goes through the mutation gateway, so each committed edit is
//! exactly one engine history entry and dirty-flagging stays correct.

use iced::Task;
use signex_engine::Command;
use uuid::Uuid;

use crate::app::Message;
use crate::app::state::{CellDraft, ModalId, Signex};

/// Keys with their own columns and commands; the engine refuses them as
/// parameters.
fn is_reserved(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "reference" | "value" | "footprint"
    )
}

impl Signex {
    /// Write one parameter value. An empty value empties the parameter, it
    /// does not delete it.
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

    /// Hold the typed text as a draft. Typing into a different cell first
    /// commits the previous one, so moving on never loses an edit.
    pub(crate) fn handle_parameter_cell_drafted(
        &mut self,
        symbol_uuid: Uuid,
        key: String,
        text: String,
    ) -> Task<Message> {
        let same_cell = self
            .ui_state
            .parameter_table
            .draft
            .as_ref()
            .is_some_and(|d| d.is_for(symbol_uuid, &key));
        if !same_cell {
            self.commit_parameter_draft();
        }
        self.ui_state.parameter_table.draft = Some(CellDraft {
            symbol_uuid,
            key,
            text,
        });
        Task::none()
    }

    /// Write the pending draft, if any, as one edit. A draft equal to the
    /// stored value writes nothing and leaves no history entry.
    pub(crate) fn commit_parameter_draft(&mut self) {
        if let Some(draft) = self.ui_state.parameter_table.draft.take() {
            self.apply_engine_command(
                Command::SetSymbolField {
                    symbol_id: draft.symbol_uuid,
                    key: draft.key,
                    value: draft.text,
                },
                false,
                false,
            );
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
        if key.is_empty() || is_reserved(&key) {
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
        self.ui_state.parameter_manager_open = false;
        self.ui_state.parameter_table = Default::default();
        self.close_detached_modal(ModalId::ParameterManager)
    }
}
