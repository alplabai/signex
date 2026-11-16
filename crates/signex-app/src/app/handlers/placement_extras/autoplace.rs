//! Autoplace Fields (#1667): the reference and value of the selected parts
//! go where the autoplacer puts them, as one undo step, fields the user moved
//! included.

use signex_types::schematic::SelectedKind;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::mutation_gateway::CommandOutcome as Applied;

impl Signex {
    /// The parts in the active selection; a selected reference or value field
    /// stands for its part.
    fn selected_part_ids(&self) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = self
            .interaction_state
            .active_canvas()
            .selected
            .iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    SelectedKind::Symbol
                        | SelectedKind::SymbolRefField
                        | SelectedKind::SymbolValField
                )
            })
            .map(|item| item.uuid)
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// The key, the palette and the menu: the selected parts. Says in the
    /// Messages panel when there is nothing to do.
    pub(super) fn autoplace_selected_fields(&mut self) {
        let ids = self.selected_part_ids();
        if ids.is_empty() {
            crate::diagnostics::log_info("Autoplace fields: select one or more parts first.");
            return;
        }
        match self.autoplace_fields_of(ids) {
            Ok(true) => {}
            Ok(false) => crate::diagnostics::log_info(
                "Autoplace fields: the fields are already where autoplace puts them.",
            ),
            Err(reason) => {
                crate::diagnostics::log_warning(format!("Autoplace fields: {reason}"));
            }
        }
    }

    /// Autoplace the fields of the parts `ids` on the sheet the edit acts on:
    /// the commit the key and the operand form share. `Ok(false)` when no
    /// field moved.
    pub(crate) fn autoplace_fields_of(&mut self, ids: Vec<Uuid>) -> Result<bool, String> {
        if self.document_state.edit_engine().is_none() {
            return Err("no schematic is open".to_string());
        }
        if let Some(reason) = self.refuse_if_active_read_only() {
            return Err(reason);
        }
        let command = signex_engine::Command::AutoplaceFields { symbol_ids: ids };
        match self.apply_engine_command_reporting(command, false, true) {
            (Applied::Changed, _) => Ok(true),
            (Applied::Unchanged, _) => Ok(false),
            (Applied::Failed, reason) => {
                Err(reason.unwrap_or_else(|| "nothing was changed".to_string()))
            }
        }
    }
}
