//! Which history a window's Undo / Redo steps, resolved once.
//!
//! The menu's enablement, the keyboard dispatch and the ERC dialog's buttons
//! all ask [`Signex::undo_target`] and read [`Signex::undo_availability`], so
//! what is lit is what runs. Rationale: ADR-0001 A2, "One undo target".

use super::DocumentKey;
use crate::app::Signex;
use crate::app::state::ProjectId;

/// What Ctrl+Z steps in one window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UndoTarget {
    /// The document the window's tab shows.
    Document(DocumentKey),
    /// The project's stored ERC rule set (the ERC dialog).
    ProjectRules(ProjectId),
    /// The window shows nothing with a history.
    None,
}

impl Signex {
    /// The history `window` (`None`: the main window) steps. A document window
    /// answers by its own tab, found by path; the main window and the detached
    /// panels and modals by the main window's active tab; a document window
    /// whose tab is gone by nothing.
    pub(crate) fn undo_target(&self, window: Option<iced::window::Id>) -> UndoTarget {
        let docs = &self.document_state;
        if self.erc_dialog_takes_input(window) {
            return docs
                .active_document_project()
                .map_or(UndoTarget::None, |p| UndoTarget::ProjectRules(p.id));
        }
        let ui = &self.ui_state;
        let bound = window
            .filter(|id| ui.main_window_id != Some(*id))
            .and_then(|id| ui.windows.get(&id))
            .and_then(|kind| docs.tab_index_of_window_kind(kind));
        let tab = match bound {
            Some(Some(index)) => docs.tabs.get(index),
            Some(None) => return UndoTarget::None,
            None => docs.tabs.get(docs.active_tab),
        };
        tab.and_then(DocumentKey::of)
            .map_or(UndoTarget::None, UndoTarget::Document)
    }

    /// Whether `window` shows a document or the ERC dialog of its own, so
    /// its Undo / Redo cannot be the main window's.
    pub(crate) fn undo_window_is_own(&self, window: Option<iced::window::Id>) -> bool {
        let ui = &self.ui_state;
        self.erc_dialog_takes_input(window)
            || window
                .filter(|id| ui.main_window_id != Some(*id))
                .and_then(|id| ui.windows.get(&id))
                .and_then(|kind| self.document_state.tab_index_of_window_kind(kind))
                .is_some()
    }

    /// `(can_undo, can_redo)` of `target`.
    pub(crate) fn undo_availability(&self, target: &UndoTarget) -> (bool, bool) {
        let docs = &self.document_state;
        match target {
            UndoTarget::None => (false, false),
            UndoTarget::ProjectRules(id) => docs
                .rules_history
                .get(id)
                .map_or((false, false), |h| (h.can_undo(), h.can_redo())),
            UndoTarget::Document(key) => match key {
                DocumentKey::Symbol(path) => {
                    docs.symbol_editors.get(path).map_or((false, false), |e| {
                        (e.history.can_undo(), e.history.can_redo())
                    })
                }
                DocumentKey::Footprint(path) => docs
                    .footprint_editors
                    .get(path)
                    .map_or((false, false), |e| {
                        (e.history.can_undo(), e.history.can_redo())
                    }),
                DocumentKey::Row(address) => self.component_history_of(address),
                DocumentKey::Project(id) => self.project_tab_history_available(*id),
                DocumentKey::Schematic(path) | DocumentKey::Pcb(path) => {
                    self.sheet_history_available(path)
                }
            },
        }
    }

    /// `(can_undo, can_redo)` of the sheet at `path`: its one timeline, whose
    /// steps are edits, net colours and net-class picks.
    fn sheet_history_available(&self, path: &std::path::Path) -> (bool, bool) {
        (
            self.sheet_has_step(path, true),
            self.sheet_has_step(path, false),
        )
    }

    /// Step `target` back (`undo`) or forward. Whether a history moved. A
    /// sheet is stepped through the active sheet: a window that shows one is
    /// made the active one first (`run_in_window`).
    pub(crate) fn step_undo_target(&mut self, target: &UndoTarget, undo: bool) -> bool {
        // The ERC dialog steps its rules through `ErcMsg::RulesUndone`/`Redone`.
        let UndoTarget::Document(key) = target else {
            return false;
        };
        match key {
            DocumentKey::Project(id) => self.step_project_tab_history(*id, undo),
            DocumentKey::Row(address) => self.step_component_editor_at(address, undo),
            DocumentKey::Symbol(path) => {
                let moved = self.editor_edit_allowed(path)
                    && self
                        .document_state
                        .symbol_editors
                        .get_mut(path)
                        .is_some_and(|e| if undo { e.undo() } else { e.redo() });
                if moved {
                    self.refresh_panel_ctx();
                }
                moved
            }
            DocumentKey::Footprint(path) => {
                let moved = self.editor_edit_allowed(path)
                    && self
                        .document_state
                        .footprint_editors
                        .get_mut(path)
                        .is_some_and(|e| if undo { e.undo() } else { e.redo() });
                if moved {
                    self.refresh_panel_ctx();
                }
                moved
            }
            DocumentKey::Schematic(_) | DocumentKey::Pcb(_) if undo => self.undo_active_sheet(),
            DocumentKey::Schematic(_) | DocumentKey::Pcb(_) => self.redo_active_sheet(),
        }
    }
}
