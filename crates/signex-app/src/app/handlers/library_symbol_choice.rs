//! Switching a placed symbol between its row's default and alternate
//! drawings (#1230).
//!
//!   1. `PanelMsg::SwitchSymbolChoice` validates against the open library's
//!      row and returns a `Task` that reads the chosen drawing off the update
//!      thread ([`resolve_symbol_choice`]).
//!   2. `SymbolChoiceResolved` registers that drawing on the sheet and
//!      re-keys the placed symbol's `lib_id` as ONE engine batch, so a single
//!      undo restores the previous drawing.
//!
//! The embedded copy of a drawing the sheet already holds is kept, as with
//! placement; drift tracking reports a copy that differs from the library.

use iced::Task;
use std::path::PathBuf;
use uuid::Uuid;

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::drift::split_lib_id;
use crate::library::messages::{LibraryMessage, ResolvedSymbolChoice};
use crate::library::place::{lib_id_of, resolve_symbol_choice};
use crate::panels::SymbolChoiceItem;
use signex_library::primitive::PrimitiveRef;
use signex_library::{ComponentRow, RowId};

impl Signex {
    /// The row a placed symbol came from, with the path and table of the
    /// open library holding it. `None` for a symbol placed without a row, or
    /// whose library is not open.
    fn row_of_placed_symbol(
        &self,
        symbol: &signex_types::schematic::Symbol,
    ) -> Option<(PathBuf, String, &ComponentRow)> {
        let (library_id, row_id) = (symbol.library_id?, symbol.row_id?);
        let library = self
            .library
            .open_libraries
            .iter()
            .find(|l| l.library_id == library_id)?;
        library.tables.iter().find_map(|(table, rows)| {
            rows.iter()
                .find(|r| r.row_id == row_id)
                .map(|row| (library.root.clone(), table.clone(), row))
        })
    }

    /// The drawings the selected placed symbol's row offers, for the
    /// Properties pick list. Empty unless the row has alternates.
    pub(crate) fn symbol_choices_for(
        &self,
        symbol: &signex_types::schematic::Symbol,
    ) -> Vec<SymbolChoiceItem> {
        let Some((_, _, row)) = self.row_of_placed_symbol(symbol) else {
            return Vec::new();
        };
        if row.alternate_symbols.is_empty() {
            return Vec::new();
        }
        row.symbol_choices()
            .into_iter()
            .map(|c| {
                let lib_id = lib_id_of(&c.symbol_ref);
                SymbolChoiceItem {
                    selected: lib_id == symbol.lib_id,
                    label: c.label,
                    lib_id,
                }
            })
            .collect()
    }

    /// Start a switch of `symbol_id` to the drawing `lib_id` names.
    pub(crate) fn handle_switch_symbol_choice(
        &mut self,
        symbol_id: Uuid,
        lib_id: &str,
    ) -> Task<Message> {
        let Some(target_sheet) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        let Some(symbol) = self
            .document_state
            .active_engine()
            .and_then(|e| e.document().symbols.iter().find(|s| s.uuid == symbol_id))
            .cloned()
        else {
            return Task::none();
        };
        if symbol.lib_id == lib_id {
            return Task::none();
        }
        let Some((library_path, table, row)) = self.row_of_placed_symbol(&symbol) else {
            self.report_symbol_choice_failure(&symbol.reference, "its library row is not open");
            return Task::none();
        };
        let chosen = split_lib_id(lib_id).map(|(l, s)| PrimitiveRef::new(l, s));
        let Some(chosen) = chosen.filter(|c| row.binds_symbol(c)) else {
            self.report_symbol_choice_failure(&symbol.reference, "its row does not offer it");
            return Task::none();
        };
        let row_id = RowId::from_uuid(row.row_id);
        let open_libraries: Vec<(Uuid, PathBuf)> = self
            .library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    resolve_symbol_choice(&library_path, &table, row_id, chosen, &open_libraries)
                })
                .await
                .unwrap_or_else(|e| Err(format!("symbol switch worker failed: {e}")))
            },
            move |result| {
                Message::Library(LibraryMessage::SymbolChoiceResolved {
                    target_sheet: target_sheet.clone(),
                    symbol_id,
                    result: result.map(Box::new),
                })
            },
        )
    }

    /// Completion of [`resolve_symbol_choice`]: embed the drawing and re-key
    /// the placed symbol as one undo step.
    pub(crate) fn handle_symbol_choice_resolved(
        &mut self,
        target_sheet: PathBuf,
        symbol_id: Uuid,
        result: Result<Box<ResolvedSymbolChoice>, String>,
    ) -> Task<Message> {
        let resolved = match result {
            Ok(resolved) => *resolved,
            Err(detail) => {
                self.report_symbol_choice_failure("symbol", &detail);
                return Task::none();
            }
        };
        let still_there = self.document_state.active_path.as_deref()
            == Some(target_sheet.as_path())
            && self
                .document_state
                .active_engine()
                .is_some_and(|e| e.document().symbols.iter().any(|s| s.uuid == symbol_id));
        if !still_there {
            self.report_symbol_choice_failure(
                "symbol",
                "the sheet or the symbol is no longer there",
            );
            return Task::none();
        }
        let commands = vec![
            signex_engine::Command::RegisterLibSymbol {
                lib_symbol: resolved.lib_symbol,
            },
            signex_engine::Command::UpdateSymbolLibId {
                symbol_id,
                lib_id: resolved.lib_id,
            },
        ];
        // An engine error is already logged by the gateway.
        self.apply_engine_commands(commands, true, true);
        Task::none()
    }

    fn report_symbol_choice_failure(&self, what: &str, detail: &str) {
        crate::diagnostics::log_warning(format!(
            "Could not switch {what} to another drawing: {detail}."
        ));
    }
}
