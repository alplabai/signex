//! The board view's layers and ratsnest (#2223). The view of each open board
//! lives in `UiState::pcb_layer_views`, for the session; nothing here touches
//! the board or its file.
//!
//! A view event acts on the board of the window it came from: the main
//! window's active board, or the board an undocked window shows.

use std::path::{Path, PathBuf};

use signex_i18n::{Localized, ids, t};
use signex_renderer::pcb::{PcbLayerView, PcbSnapshot, board_copper_layers};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use crate::app::state::WindowKind;
use crate::app::{PcbViewMsg, Signex};
use crate::pcb_canvas::layer_style;

impl Signex {
    /// Apply a board-view event to the board of `window` (`None`: the main
    /// window's active board), then redraw that board.
    pub(crate) fn apply_pcb_view_event(
        &mut self,
        window: Option<iced::window::Id>,
        event: PcbViewMsg,
    ) {
        self.edit_pcb_view(window, |view, copper| apply_event(view, copper, event));
    }

    /// What the view of the board at `path` shows of its layers; the default
    /// view for a board nobody changed.
    pub(crate) fn pcb_layer_view(&self, path: &Path) -> PcbLayerView {
        self.ui_state
            .pcb_layer_views
            .get(path)
            .cloned()
            .unwrap_or_default()
    }

    /// The snapshot of `board` as the view of `path` shows it, without the
    /// ratsnest.
    pub(crate) fn pcb_snapshot_for(&self, path: &Path, board: &PcbBoard) -> PcbSnapshot {
        PcbSnapshot::from_board_with_view(board, &self.pcb_layer_view(path))
    }

    /// The path of the board an undocked window shows, if it shows one.
    pub(crate) fn undocked_pcb_path(&self, window: iced::window::Id) -> Option<PathBuf> {
        if self.ui_state.main_window_id == Some(window) {
            return None;
        }
        let Some(WindowKind::UndockedTab { path, .. }) = self.ui_state.windows.get(&window) else {
            return None;
        };
        self.pcb_for_window(window).map(|_| path.clone())
    }

    /// The board a view event from `window` acts on: an undocked window's
    /// own board, else the main window's active one.
    fn pcb_view_target(&self, window: Option<iced::window::Id>) -> Option<(PathBuf, &PcbBoard)> {
        if let Some(path) = window.and_then(|id| self.undocked_pcb_path(id)) {
            let board = self.document_state.pcb_board(&path)?;
            return Some((path, board));
        }
        let path = self.active_tab_path()?;
        Some((path, self.active_pcb()?))
    }

    /// The status bar pill of `window`: the active copper layer of the board
    /// that window shows. No pill while every copper layer is hidden.
    pub(crate) fn status_active_layer(
        &self,
        window: iced::window::Id,
    ) -> Option<(Localized, Localized)> {
        let shows_own_board = self.ui_state.main_window_id == Some(window)
            || self.undocked_pcb_path(window).is_some();
        if !shows_own_board {
            return None;
        }
        let (path, _) = self.pcb_view_target(Some(window))?;
        let layer = self.pcb_layer_view(&path).shown_active()?;
        let i18n = &self.ui_state.i18n;
        Some((
            t!(i18n, ids::PCB_STATUS_LAYER),
            layer_style::copper_name(i18n, layer),
        ))
    }

    fn edit_pcb_view(
        &mut self,
        window: Option<iced::window::Id>,
        edit: impl FnOnce(&mut PcbLayerView, &[CopperLayer]),
    ) {
        let Some((path, board)) = self.pcb_view_target(window) else {
            return;
        };
        let copper = board_copper_layers(&board.layers);
        let view = self
            .ui_state
            .pcb_layer_views
            .entry(path.clone())
            .or_default();
        edit(view, &copper);
        view.settle_active(&copper);
        self.redraw_pcb_view_of(&path);
        self.refresh_panel_ctx();
    }

    /// Redraw every canvas showing the board at `path` after its view changed.
    fn redraw_pcb_view_of(&mut self, path: &Path) {
        if self.active_pcb().is_some() && self.active_tab_path().as_deref() == Some(path) {
            // The main canvas, its selection index and the undocked windows
            // showing the same tab.
            self.sync_pcb_canvas_from_visible_board();
            return;
        }
        let Some(snapshot) = self
            .document_state
            .pcb_board(path)
            .map(|board| self.pcb_snapshot_for(path, board))
        else {
            return;
        };
        let windows: Vec<iced::window::Id> = self
            .interaction_state
            .pcb_canvases
            .keys()
            .copied()
            .filter(|id| self.undocked_pcb_path(*id).as_deref() == Some(path))
            .collect();
        for id in windows {
            if let Some(canvas) = self.interaction_state.pcb_canvases.get_mut(&id) {
                canvas.set_renderer_snapshot(Some(snapshot.clone()));
                canvas.clear_content_cache();
            }
        }
    }
}

/// What `event` changes in `view`; `copper` is the board's copper, top to bottom.
fn apply_event(view: &mut PcbLayerView, copper: &[CopperLayer], event: PcbViewMsg) {
    match event {
        PcbViewMsg::RatsnestVisibilityToggled => view.toggle_ratsnest(),
        PcbViewMsg::ActiveLayerStepped { forward } => view.step_active(copper, forward),
        PcbViewMsg::ActiveLayerPicked(layer) => view.set_active(layer),
        PcbViewMsg::LayerVisibilityToggled(layer) => view.toggle_visibility(layer),
        PcbViewMsg::ActiveLayerVisibilityToggled => {
            view.toggle_visibility(PcbLayer::Copper(view.active()));
        }
        PcbViewMsg::AllLayersShown => view.show_all(),
    }
}

#[cfg(test)]
#[path = "pcb_view_tests.rs"]
mod tests;
