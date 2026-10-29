//! Switching a placed symbol between its row's default and alternate
//! drawings (#1230).
//!
//!   1. `PanelMsg::SwitchSymbolChoice` validates against the open library's
//!      row and returns a `Task` that reads the chosen drawing off the update
//!      thread ([`resolve_symbol_choice`]).
//!   2. `SymbolChoiceResolved` registers that drawing on the sheet and
//!      re-keys the placed symbol's `lib_id` as ONE engine batch, so a single
//!      undo restores the previous drawing. A result that is not the latest
//!      request for its symbol, or that belongs to a closed or re-opened
//!      sheet, is dropped and logged. A drawing that would move a connected
//!      pin is refused and logged: the wire would dangle.
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

/// What the off-thread read of a switch carries back to `update`.
pub(crate) struct SwitchContext {
    pub(crate) target_sheet: PathBuf,
    pub(crate) engine_instance: u64,
    pub(crate) symbol_id: Uuid,
    pub(crate) request: u64,
    pub(crate) label: String,
}

impl Signex {
    /// The row a placed symbol came from, with the path and table of the
    /// open library holding it. `None` for a symbol placed without a row, or
    /// whose library is not open.
    pub(crate) fn row_of_placed_symbol(
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
    /// Properties pick list. Empty unless the row has alternates, or the
    /// symbol sits on a drawing its row no longer offers: that one is listed
    /// as "(no longer offered)" so the default can still be picked.
    pub(crate) fn symbol_choices_for(
        &self,
        symbol: &signex_types::schematic::Symbol,
    ) -> Vec<SymbolChoiceItem> {
        let Some((_, _, row)) = self.row_of_placed_symbol(symbol) else {
            return Vec::new();
        };
        let mut items: Vec<SymbolChoiceItem> = row
            .symbol_choices()
            .into_iter()
            .map(|c| {
                let lib_id = lib_id_of(&c.symbol_ref);
                SymbolChoiceItem {
                    selected: lib_id == symbol.lib_id,
                    label: c.label,
                    lib_id,
                }
            })
            .collect();
        if !items.iter().any(|i| i.selected) {
            items.push(SymbolChoiceItem {
                lib_id: symbol.lib_id.clone(),
                label: signex_library::NO_LONGER_OFFERED_LABEL.to_string(),
                selected: true,
            });
        }
        if items.len() < 2 {
            return Vec::new();
        }
        items
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
        let label = row
            .symbol_choices()
            .into_iter()
            .find(|c| c.symbol_ref == chosen)
            .map(|c| c.label)
            .unwrap_or_default();
        let engine_instance = self
            .document_state
            .active_engine()
            .map_or(0, |e| e.instance_id());
        let request = self.library.symbol_choice_requests.begin(symbol_id);
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
                    engine_instance,
                    symbol_id,
                    request,
                    label: label.clone(),
                    result: result.map(Box::new),
                })
            },
        )
    }

    /// Completion of [`resolve_symbol_choice`]: embed the drawing and re-key
    /// the placed symbol as one undo step.
    pub(crate) fn handle_symbol_choice_resolved(
        &mut self,
        ctx: SwitchContext,
        result: Result<Box<ResolvedSymbolChoice>, String>,
    ) -> Task<Message> {
        let SwitchContext {
            target_sheet,
            engine_instance,
            symbol_id,
            request,
            label,
        } = ctx;
        let reference = self.reference_of(&target_sheet, symbol_id);
        // Whatever the outcome, the latest request for the symbol is over.
        let latest = self
            .library
            .symbol_choice_requests
            .is_latest(symbol_id, request);
        if latest {
            self.library.symbol_choice_requests.finish(symbol_id);
        }
        let resolved = match result {
            Ok(resolved) => *resolved,
            Err(detail) => {
                self.report_symbol_choice_failure(&reference, &detail);
                return Task::none();
            }
        };
        if !latest {
            self.report_symbol_choice_failure(
                &reference,
                &format!("a newer switch of it was started, so the {label} drawing was dropped"),
            );
            return Task::none();
        }
        let same_sheet = self.document_state.active_path.as_deref() == Some(target_sheet.as_path())
            && self
                .document_state
                .active_engine()
                .is_some_and(|e| e.instance_id() == engine_instance);
        if !same_sheet {
            self.report_symbol_choice_failure(
                &reference,
                "the sheet is no longer active or was reopened; pick again",
            );
            return Task::none();
        }
        let Some(doc) = self.document_state.active_engine().map(|e| e.document()) else {
            return Task::none();
        };
        if !doc.symbols.iter().any(|s| s.uuid == symbol_id) {
            self.report_symbol_choice_failure(
                &reference,
                "the symbol was deleted; nothing switched",
            );
            return Task::none();
        }
        // The copy the sheet already holds wins over the fresh one (as with
        // placement), so it is the geometry the symbol would end up with.
        let effective = doc
            .lib_symbols
            .get(&resolved.lib_id)
            .unwrap_or(&resolved.lib_symbol);
        let blockers = signex_engine::switch_blockers(doc, symbol_id, effective);
        if let Some(why) = switch_refusal(&blockers) {
            crate::diagnostics::log_warning(format!(
                "{reference}: switching to {label} would {why}; not switched."
            ));
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

    /// The reference designator of `symbol_id` on `sheet`, or "symbol" when
    /// it cannot be found.
    fn reference_of(&self, sheet: &std::path::Path, symbol_id: Uuid) -> String {
        self.document_state
            .engines
            .get(sheet)
            .and_then(|e| e.document().symbols.iter().find(|s| s.uuid == symbol_id))
            .map_or_else(|| "symbol".to_string(), |s| s.reference.clone())
    }

    fn report_symbol_choice_failure(&self, reference: &str, detail: &str) {
        crate::diagnostics::log_warning(format!(
            "Could not switch {reference} to another drawing: {detail}."
        ));
    }
}

/// What a refused switch would do, as `disconnect pin 1 and connect pin 2`;
/// `None` when nothing blocks it.
fn switch_refusal(blockers: &signex_engine::SwitchBlockers) -> Option<String> {
    let pins = |keys: &[String]| {
        let noun = if keys.len() == 1 { "pin" } else { "pins" };
        format!("{noun} {}", keys.join(", "))
    };
    let mut parts = Vec::new();
    if let Some(unit) = blockers.missing_unit {
        parts.push(format!("draw nothing for unit {unit}"));
    }
    if !blockers.lost.is_empty() {
        parts.push(format!("disconnect {}", pins(&blockers.lost)));
    }
    if !blockers.gained.is_empty() {
        parts.push(format!("connect {}", pins(&blockers.gained)));
    }
    (!parts.is_empty()).then(|| parts.join(" and "))
}
