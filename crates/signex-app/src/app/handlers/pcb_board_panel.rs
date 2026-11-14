//! The board Properties panel with nothing selected (#2353). The selection
//! filter and the snap options are editor settings for the session
//! (`UiState`); the snap grid step is the board's own and changes through
//! one `BoardCommand::SetGridSize` undo step.

use iced::Task;
use pcb_geom::{ItemRef, Side};
use signex_engine::pcb::BoardCommand;
use signex_i18n::{ids, t};
use signex_types::measure::parse_length_field_mm;

use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, OverlayMsg, Signex};
use crate::panels::{BoardTab, ComponentSide, PanelKind, PcbBoardMsg, pcb_board_properties};

impl Signex {
    pub(crate) fn handle_pcb_board_panel(&mut self, msg: &PcbBoardMsg) -> Task<Message> {
        let mut task = Task::none();
        let view = &mut self.ui_state.pcb_board_panel;
        match msg {
            // Boards hold no parameters yet: that tab stays disabled.
            PcbBoardMsg::TabPicked(BoardTab::Parameters) => {}
            PcbBoardMsg::TabPicked(tab) => view.tab = *tab,
            PcbBoardMsg::SearchChanged(text) => view.search = text.clone(),
            PcbBoardMsg::FilterMenuToggled => view.filter_menu_open = !view.filter_menu_open,
            PcbBoardMsg::GridStepTyped(text) => view.grid_draft = Some(text.clone()),
            PcbBoardMsg::GridStepSubmitted => self.apply_pcb_grid_step(),
            PcbBoardMsg::FilterKindToggled(kind) => {
                self.ui_state.pcb_pick_filter.toggle(*kind);
                self.refresh_pcb_highlight();
            }
            PcbBoardMsg::FilterAllToggled => {
                let filter = &mut self.ui_state.pcb_pick_filter;
                let all = filter.is_all_on();
                filter.set_all(!all);
                self.refresh_pcb_highlight();
            }
            PcbBoardMsg::SnapToGridsToggled => {
                let snap = &mut self.ui_state.pcb_snap;
                snap.to_grids = !snap.to_grids;
            }
            PcbBoardMsg::SnapScopePicked(scope) => self.ui_state.pcb_snap.scope = *scope,
            PcbBoardMsg::SnapObjectToggled(object) => self.ui_state.pcb_snap.toggle_object(*object),
            PcbBoardMsg::SnapDistancePicked(distance) => {
                self.ui_state.pcb_snap.distance = *distance;
            }
            PcbBoardMsg::ComponentsSelected(side) => self.select_pcb_components(*side),
            PcbBoardMsg::DrcResultsOpened => {
                // The check's results are listed in the Messages panel.
                task = Task::done(Message::Overlay(OverlayMsg::OpenPanel(PanelKind::Messages)));
            }
        }
        self.sync_pcb_board_summary();
        task
    }

    /// Shift+E on a board: step Snapping through All Layers, Current Layer
    /// and Off.
    pub(crate) fn cycle_pcb_snapping(&mut self) {
        let snap = &mut self.ui_state.pcb_snap;
        snap.scope = snap.scope.next();
        self.sync_pcb_board_summary();
    }

    /// Select the board's components on `side`, as a Board Information count
    /// asks. Those on a hidden layer are left out.
    fn select_pcb_components(&mut self, side: ComponentSide) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let items: Vec<ItemRef> = board
            .footprints
            .iter()
            .filter(|fp| match side {
                ComponentSide::All => true,
                ComponentSide::Top => Side::of_layer(&fp.layer) == Side::Top,
                ComponentSide::Bottom => Side::of_layer(&fp.layer) == Side::Bottom,
            })
            .map(|fp| ItemRef::Footprint(fp.uuid))
            .collect();
        let done = self.with_pcb_selection(|sel, board| sel.select_items(board, &items));
        if done.is_some_and(|d| d.changed) {
            self.refresh_pcb_highlight();
        }
    }

    /// Apply the typed grid step, in the panel's unit, as one undo step; a
    /// step that is not a length above zero, or a refused edit, is said in
    /// the Messages panel and changes nothing.
    fn apply_pcb_grid_step(&mut self) {
        let Some(text) = self.ui_state.pcb_board_panel.grid_draft.take() else {
            return;
        };
        let Some(path) = self.active_tab_path() else {
            return;
        };
        let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
        let refused = |why: String| {
            let line = t!(i18n, ids::PCB_BOARD_GRID_REFUSED, "why" => why);
            crate::diagnostics::log_warning(line.into_string());
        };
        let unit = pcb_board_properties::shown_unit(self.ui_state.unit);
        let Some(grid_mm) = parse_length_field_mm(&text, unit).filter(|mm| *mm > 0.0) else {
            refused(t!(i18n, ids::PCB_BOARD_GRID_INVALID).into_string());
            return;
        };
        let Some(engine) = self.document_state.board_engine(&path) else {
            return;
        };
        let label = t!(i18n, ids::PCB_BOARD_GRID_UNDO).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: engine.revision(),
        };
        let command = BoardCommand::SetGridSize { grid_mm };
        if let Err(error) = self.execute_board_commands(&path, vec![command], Some(step)) {
            refused(error.to_string());
        }
    }
}

#[cfg(test)]
#[path = "pcb_board_panel_tests.rs"]
mod tests;
