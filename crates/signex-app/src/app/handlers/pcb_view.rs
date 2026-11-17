//! The board view's layers and ratsnest (#2223). The view of each open board
//! lives in `UiState::pcb_layer_views`, for the session; nothing here touches
//! the board or its file.
//!
//! A view event acts on the board of the window it came from: the main
//! window's active board, or the board an undocked window shows.

use std::path::{Path, PathBuf};

use signex_i18n::{Localized, ids, t};
use signex_renderer::pcb::{
    BoardObject, BuiltinLayerSet, LayerSetId, PcbLayerView, PcbSnapshot, ShownLayerSet, board_copper_layers,
    board_layers, builtin_set_layers, kind_layers, listed_layers, signal_layers,
};
use signex_types::pcb::{LayerDef, PcbBoard};
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

    /// The view of the board a menu opened in `window` acts on (`None`: the
    /// main window's active board); the default view without a board.
    pub(crate) fn pcb_layer_view_of_window(
        &self,
        window: Option<iced::window::Id>,
    ) -> PcbLayerView {
        self.pcb_view_target(window)
            .map(|(path, _)| self.pcb_layer_view(&path))
            .unwrap_or_default()
    }

    /// The active copper layer of the board in view; the top layer without
    /// one, or while a non-copper layer is active. Object snapping on the
    /// current layer reads it (#2353).
    pub(crate) fn pcb_active_layer(&self) -> CopperLayer {
        self.active_tab_path()
            .and_then(|path| self.pcb_layer_view(&path).active().copper())
            .unwrap_or(CopperLayer::Top)
    }

    /// The snapshot of `board` as the view of `path` shows it, without the
    /// ratsnest.
    pub(crate) fn pcb_snapshot_for(&self, path: &Path, board: &PcbBoard) -> PcbSnapshot {
        PcbSnapshot::from_board_with_view(board, &self.pcb_layer_view(path))
    }

    /// The snapshot an undocked window draws of the board at `path`: its
    /// view, with the ratsnest unless that view hides it.
    pub(crate) fn undocked_pcb_snapshot(&self, path: &Path, board: &PcbBoard) -> PcbSnapshot {
        let fills = pcb_geom::zone_fills(board);
        let snapshot = self.pcb_snapshot_for(path, board).with_zone_fills(&fills);
        if !self.pcb_layer_view(path).ratsnest_visible() {
            return snapshot;
        }
        self.interaction_state
            .pcb_ratsnest
            .apply_uncached(path, board, &fills, snapshot)
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

    /// The status bar pill of `window`: the active layer of the board that
    /// window shows. No pill while that layer is hidden.
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
            layer_style::layer_name(i18n, layer),
        ))
    }

    /// Edit the view of the board of `window` with its stackup, settle its
    /// active layer and redraw it.
    pub(crate) fn edit_pcb_view(
        &mut self,
        window: Option<iced::window::Id>,
        edit: impl FnOnce(&mut PcbLayerView, &[LayerDef]),
    ) {
        let Some((path, board)) = self.pcb_view_target(window) else {
            return;
        };
        let layers = board.layers.clone();
        self.edit_pcb_view_with(path, &layers, edit);
    }

    /// [`Self::edit_pcb_view`] on the open board at `path`, wherever it is
    /// shown (a command that names its document, #2394). Whether the board
    /// is open.
    pub(crate) fn edit_pcb_view_at(
        &mut self,
        path: &Path,
        edit: impl FnOnce(&mut PcbLayerView, &[LayerDef]),
    ) -> bool {
        let Some(board) = self.document_state.pcb_board(path) else {
            return false;
        };
        let layers = board.layers.clone();
        self.edit_pcb_view_with(path.to_path_buf(), &layers, edit);
        true
    }

    fn edit_pcb_view_with(
        &mut self,
        path: PathBuf,
        layers: &[LayerDef],
        edit: impl FnOnce(&mut PcbLayerView, &[LayerDef]),
    ) {
        let view = self
            .ui_state
            .pcb_layer_views
            .entry(path.clone())
            .or_default();
        edit(view, layers);
        settle(view, layers);
        self.redraw_pcb_view_of(&path);
        self.refresh_panel_ctx();
    }

    /// Apply a board-view event to the open board at `path` (#2394).
    pub(crate) fn apply_pcb_view_event_at(&mut self, path: &Path, event: PcbViewMsg) -> bool {
        self.edit_pcb_view_at(path, |view, layers| apply_event(view, layers, event))
    }

    /// Redraw every canvas showing the board at `path` after its view changed.
    fn redraw_pcb_view_of(&mut self, path: &Path) {
        if self.active_pcb().is_some() && self.active_tab_path().as_deref() == Some(path) {
            // The main canvas, its selection index and the undocked windows
            // showing the same tab.
            self.sync_pcb_canvas_from_visible_board();
        } else {
            self.repaint_undocked_boards(path);
        }
        // An undocked window's selection drops what the view now hides.
        self.refresh_undocked_pcb_highlights(path);
    }
}

/// A hidden active layer moves on: copper to the next shown copper layer,
/// any other layer to the next shown layer of the board. While a layer set
/// is shown it moves into the set (#2394): to any shown layer of the board
/// when the set has no layer of its kind.
fn settle(view: &mut PcbLayerView, layers: &[LayerDef]) {
    let candidates = if view.active().is_copper() {
        copper_layers(layers)
    } else {
        board_layers(layers)
    };
    view.settle_active(&candidates);
    if view.layer_set().is_some() {
        view.settle_active(&board_layers(layers));
    }
}

fn copper_layers(layers: &[LayerDef]) -> Vec<PcbLayer> {
    board_copper_layers(layers)
        .into_iter()
        .map(PcbLayer::Copper)
        .collect()
}

/// What `event` changes in `view`; `layers` is the board's stackup.
fn apply_event(view: &mut PcbLayerView, layers: &[LayerDef], event: PcbViewMsg) {
    match event {
        PcbViewMsg::RatsnestVisibilityToggled => view.toggle_ratsnest(),
        PcbViewMsg::OriginMarkerVisibilityToggled => view.toggle_origin_marker(),
        PcbViewMsg::ActiveLayerStepped { forward } => {
            view.step_active(&board_layers(layers), forward);
        }
        PcbViewMsg::SignalLayerStepped { forward } => {
            view.step_active(&signal_layers(layers), forward);
        }
        PcbViewMsg::ActiveLayerPicked(layer) => view.set_active(layer),
        PcbViewMsg::SingleLayerModeToggled => view.toggle_single_layer(),
        PcbViewMsg::LayerVisibilityToggled(layer) => view.toggle_visibility(layer),
        PcbViewMsg::ActiveLayerVisibilityToggled => view.toggle_visibility(view.active()),
        PcbViewMsg::MultiLayerVisibilityToggled => view.toggle_multi_layer(),
        PcbViewMsg::AllLayersShown => view.show_all(),
        PcbViewMsg::LayerHidden(layer) => view.hide(layer),
        PcbViewMsg::OnlyLayerShown(layer) => view.show_only(&listed_layers(layers), layer),
        PcbViewMsg::LayerHighlightToggled(layer) => view.toggle_highlight(layer),
        PcbViewMsg::LayerKindVisibilitySet { kind, visible } => {
            view.set_layers_visible(&kind_layers(layers, kind), visible);
        }
        PcbViewMsg::BuiltinLayerSetShown(BuiltinLayerSet::All) => view.show_all(),
        PcbViewMsg::BuiltinLayerSetShown(set) => {
            let shown = ShownLayerSet {
                id: LayerSetId::Builtin(set),
                layers: builtin_set_layers(layers, set),
            };
            view.show_set(&listed_layers(layers), shown);
        }
        PcbViewMsg::BoardFlipToggled => view.toggle_flipped(),
        PcbViewMsg::LayerVisibilitySet { layer, visible } => {
            view.set_layers_visible(&[layer], visible);
        }
        PcbViewMsg::LayerColourSet { layer, colour } => view.set_layer_colour(layer, colour),
        PcbViewMsg::UsedLayersOnlySet(on) => view.set_used_only(on),
        PcbViewMsg::ObjectVisibilitySet { kind, visible } => {
            let kinds = match kind {
                Some(kind) => vec![kind],
                None => BoardObject::ALL.into_iter().filter(|k| k.can_hide()).collect(),
            };
            for kind in kinds {
                view.set_object_visible(kind, visible);
            }
        }
        PcbViewMsg::ObjectTransparencySet { kind, percent } => {
            view.set_object_transparency(kind, percent);
        }
    }
}

#[cfg(test)]
#[path = "pcb_view_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pcb_layer_tabs_tests.rs"]
mod layer_tabs_tests;

#[cfg(test)]
#[path = "pcb_layer_sets_tests.rs"]
mod layer_sets_tests;
