//! The table's own controls (#2371): the focused row Space ticks, the
//! filters and folds, and a problem row's Choose footprint..., which sets
//! the part's footprint on the schematic and reads the list again.

use std::path::PathBuf;

use iced::Task;
use signex_engine::pcb::eco::EcoKey;
use signex_i18n::ids;
use signex_library::{PrimitiveKind, PrimitiveRef};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::table::TableState;
use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::place::lib_id_of;
use crate::library::state::PrimitivePickerTarget;

impl Signex {
    /// Change the table's view choices; nothing in the list changes.
    pub(super) fn with_update_pcb_table(&mut self, change: impl FnOnce(&mut TableState)) {
        if let Some(state) = self.ui_state.update_pcb.as_mut() {
            change(&mut state.table);
        }
    }

    pub(super) fn focus_update_pcb_row(&mut self, key: EcoKey) {
        self.with_update_pcb_table(|table| table.focused = Some(key));
    }

    /// Space: tick or untick the focused row, when it can be ticked.
    pub(super) fn toggle_focused_update_pcb_row(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return Task::none();
        };
        let focused = state.table.focused.as_ref();
        let Some(row) = focused.and_then(|key| state.ready_list()?.row(key)) else {
            return Task::none();
        };
        if row.blocked.is_some() {
            return Task::none();
        }
        let (key, on) = (row.key.clone(), !row.on);
        self.tick_update_pcb(|list| list.set_on(&key, on))
    }

    /// Choose footprint...: the footprint picker opens for the part of an
    /// Add row.
    pub(super) fn choose_update_pcb_footprint(&mut self, key: &EcoKey) -> Task<Message> {
        let EcoKey::AddFootprint { sheet, part } = key else {
            return Task::none();
        };
        Task::done(Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Footprint,
            target: PrimitivePickerTarget::DesignPart {
                sheet: sheet.clone(),
                part: *part,
            },
        }))
    }

    /// The picker picked `footprint` for the part: set it on every placed
    /// unit as one undo step, then read the list again so the row re-checks.
    /// A part whose library row offers the footprint is built with that
    /// binding; any other part names it by its library key.
    pub(super) fn set_update_pcb_footprint(
        &mut self,
        sheet: &InstancePath,
        part: Uuid,
        footprint: PrimitiveRef,
    ) -> Task<Message> {
        let Some(path) = self.update_pcb_part_sheet(sheet, part) else {
            self.log_update_pcb_fix(ids::UPDATE_PCB_FIX_SHEET_CLOSED);
            return Task::none();
        };
        let binding = lib_id_of(&footprint);
        let Some(doc) = self.document_state.engines.get(&path).map(|e| e.document()) else {
            return Task::none();
        };
        let commands = self
            .footprint_binding_commands(doc, part, &binding)
            .unwrap_or_else(|_| {
                signex_engine::placed_unit_ids(doc, part)
                    .into_iter()
                    .map(|symbol_id| signex_engine::Command::UpdateSymbolFootprint {
                        symbol_id,
                        footprint: binding.clone(),
                    })
                    .collect()
            });
        self.activate_tab_for_path(&path);
        if !self.apply_engine_commands(commands, true, true) {
            self.log_update_pcb_fix(ids::UPDATE_PCB_FIX_NOT_SET);
            return Task::none();
        }
        let previous = self
            .ui_state
            .update_pcb
            .as_ref()
            .and_then(|s| s.list.clone());
        self.start_update_pcb_read(previous)
    }

    /// The open sheet holding the part's anchor unit, among the sheets the
    /// list was read from.
    fn update_pcb_part_sheet(&self, sheet: &InstancePath, part: Uuid) -> Option<PathBuf> {
        let state = self.ui_state.update_pcb.as_ref()?;
        let snapshot = state.snapshot.as_ref()?;
        let holds = |path: &PathBuf| {
            self.document_state
                .engines
                .get(path)
                .is_some_and(|e| e.document().symbols.iter().any(|s| s.uuid == part))
        };
        let mut candidates = snapshot
            .sheet_revisions
            .iter()
            .map(|(path, _)| path)
            .filter(|path| holds(path));
        let first = candidates.next()?.clone();
        // Of several files with the unit, the one the placement shows.
        let placed: Vec<PathBuf> = self
            .hierarchy_describing(&first)
            .map(|(h, _)| h.paths_at(sheet).into_iter().map(PathBuf::from).collect())
            .unwrap_or_default();
        Some(
            std::iter::once(first.clone())
                .chain(candidates.cloned())
                .find(|path| placed.contains(path))
                .unwrap_or(first),
        )
    }

    fn log_update_pcb_fix(&self, id: signex_i18n::MessageId) {
        // The log stays English under any UI language.
        crate::diagnostics::log_warning(signex_i18n::I18n::en_us().message_plain(id, &[]));
    }
}
