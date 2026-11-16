//! The PCB parts of the panel context: the Layers panel and the Properties
//! board summary of the active board tab.

use super::super::*;

impl Signex {
    /// The Layers panel: the active board's rows, hidden layers and active layer.
    pub(super) fn pcb_layers_panel_state(&self) -> crate::panels::PcbLayersPanel {
        let Some(board) = self.active_pcb() else {
            return crate::panels::PcbLayersPanel {
                i18n: self.ui_state.i18n.clone(),
                ..Default::default()
            };
        };
        let view = self
            .active_tab_path()
            .map(|path| self.pcb_layer_view(&path))
            .unwrap_or_default();
        crate::panels::PcbLayersPanel::new(&board.layers, &view, self.ui_state.i18n.clone())
    }

    /// The Properties panel's board summary for the active tab. The selection
    /// counts only once it is aligned with that tab: right after a tab
    /// switch it may still hold another board's items.
    pub(super) fn pcb_board_summary(&self) -> Option<crate::panels::PcbBoardSummary> {
        let board = self.active_pcb_snapshot()?;
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let selection = &self.interaction_state.pcb_selection;
        let selected = if selection.document() == Some(tab.path.as_path()) {
            selection.selection().len()
        } else {
            0
        };
        let footprint = self.pcb_footprint_facts(&tab.path, board);
        // The totals are counted once per board revision (#2353).
        let revision = self
            .document_state
            .pcb_revisions
            .get(&tab.path)
            .copied()
            .unwrap_or(0);
        let key = (tab.path.clone(), revision);
        let stats = self
            .document_state
            .panel_ctx
            .pcb_board
            .as_ref()
            .filter(|s| s.stats_key.as_ref() == Some(&key))
            .map_or_else(|| pcb_geom::BoardStats::of(board), |s| s.stats.clone());
        let ratsnest = &self.interaction_state.pcb_ratsnest;
        let summary = crate::panels::PcbBoardSummary {
            unrouted_nets: ratsnest.unrouted_nets(&key),
            hidden_nets: ratsnest.hidden_net_count(&tab.path),
            filter: self.ui_state.pcb_pick_filter.clone(),
            presets: self.ui_state.pcb_filter_presets.clone(),
            preset_tab: self.ui_state.pcb_filter_preset_tab,
            snap: self.ui_state.pcb_snap.clone(),
            fast_grids_mm: self.ui_state.fast_grids_mm,
            view: self.ui_state.pcb_board_panel.clone(),
            stats_key: Some(key),
            ..crate::panels::PcbBoardSummary::with_stats(board, tab.title.clone(), selected, stats)
        };
        let component = self.pcb_component_facts(&tab.path, board);
        let primitive = self.pcb_primitive_facts(&tab.path, board);
        Some(
            summary
                .with_footprint(footprint)
                .with_component(component)
                .with_primitive(primitive),
        )
    }

    /// Bring the board summary in step with a selection change, without a
    /// full rebuild. Inside an undocked window's event the window's context
    /// and selection are the ones in the main slots (`run_in_window`).
    pub(crate) fn sync_pcb_board_summary(&mut self) {
        let summary = self.pcb_board_summary();
        // A draft belongs to the footprint Properties shows on its board; once
        // another selection on THAT board replaces it, the unapplied text is
        // dropped. A selection on another board (another window) leaves it.
        let in_view = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());
        let shown = summary
            .as_ref()
            .and_then(|s| s.footprint.as_ref())
            .map(|f| f.uuid);
        let draft = self.interaction_state.pcb_footprint_draft.as_ref();
        if draft.is_some_and(|d| {
            in_view.as_deref() == Some(d.board.as_path()) && shown != Some(d.footprint)
        }) {
            self.interaction_state.pcb_footprint_draft = None;
        }
        self.drop_moved_pcb_component_draft(summary.as_ref().and_then(|s| s.component.as_ref()));
        if let Some(board) = in_view.as_deref() {
            let shown = summary.as_ref().and_then(|s| s.primitive.as_ref());
            let target = shown.map(|p| &p.common().target);
            self.interaction_state
                .pcb_primitive
                .keep_only(board, target);
        }
        self.document_state.panel_ctx.pcb_board = summary;
    }
}
