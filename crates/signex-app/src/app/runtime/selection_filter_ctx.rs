//! The filter panels' part of the panel context (Graphite mockup #55,
//! Graphite mockup #56): the editor the Filter panel follows and its
//! per-kind counts, taken once per document revision, and the Filter panel's
//! preset rename and edited preset, read from the Model.

use super::super::*;
use crate::panels::{FilterEditor, KindCounts, SelectionFilterFacts};

impl Signex {
    pub(super) fn selection_filter_facts(&self) -> SelectionFilterFacts {
        let previous = &self.document_view.panel_ctx.selection_filter;
        let facts = self
            .board_filter_facts(previous)
            .or_else(|| self.schematic_filter_facts(previous))
            .unwrap_or_default();
        SelectionFilterFacts {
            rename: self.filter_preset_rename(),
            editing: self.edited_custom_filter(),
            ..facts
        }
    }

    /// The schematic preset selected for editing, while it still exists.
    pub(in crate::app) fn edited_custom_filter(&self) -> Option<usize> {
        let presets = self.interaction_state.custom_filter_presets.len();
        self.interaction_state
            .editing_custom_filter
            .filter(|idx| *idx < presets)
    }

    /// The Filter panel's preset rename, while its preset still exists.
    pub(in crate::app) fn filter_preset_rename(&self) -> Option<(usize, String)> {
        let presets = self.interaction_state.custom_filter_presets.len();
        self.interaction_state
            .filter_preset_rename
            .clone()
            .filter(|(idx, _)| *idx < presets)
    }

    /// A board tab: its kinds counted, or kept from `previous` at the same
    /// board revision.
    fn board_filter_facts(&self, previous: &SelectionFilterFacts) -> Option<SelectionFilterFacts> {
        let board = self.active_pcb()?;
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let revision = self
            .document_state
            .pcb_revisions
            .get(&tab.path)
            .copied()
            .unwrap_or(0);
        let key = Some((tab.path.clone(), revision));
        let kept = previous.editor == FilterEditor::Board && previous.key == key;
        Some(SelectionFilterFacts {
            editor: FilterEditor::Board,
            board: if kept {
                previous.board.clone()
            } else {
                crate::panels::board_counts(board)
            },
            key,
            ..SelectionFilterFacts::default()
        })
    }

    /// A schematic tab: its sheet's kinds counted, or kept from `previous`
    /// at the same engine revision.
    fn schematic_filter_facts(
        &self,
        previous: &SelectionFilterFacts,
    ) -> Option<SelectionFilterFacts> {
        if !self.has_active_schematic() {
            return None;
        }
        let snapshot = self.active_render_snapshot()?;
        let path = self.document_state.active_path.clone()?;
        let revision = self
            .document_state
            .engines
            .get(&path)
            .map(signex_engine::Engine::revision)?;
        let key = Some((path, revision));
        let kept = previous.editor == FilterEditor::Schematic && previous.key == key;
        let schematic: KindCounts<crate::active_bar::SelectionFilter> = if kept {
            previous.schematic.clone()
        } else {
            crate::app::handlers::selection_workflow::selection_filter_counts(snapshot)
        };
        Some(SelectionFilterFacts {
            editor: FilterEditor::Schematic,
            schematic,
            key,
            ..SelectionFilterFacts::default()
        })
    }
}
