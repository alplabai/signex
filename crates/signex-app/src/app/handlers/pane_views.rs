//! Each pane keeps its own view (mockup row 207, #1307). The focused pane
//! draws on the main canvases and every other pane on a canvas of its own
//! (`InteractionState::tile_panes`). When the focus moves to another pane,
//! the main canvases take that pane's pan and zoom, and the pane left
//! behind keeps the view the main canvases had; two views of one sheet swap
//! their selections too, so the Properties panel follows the focused pane.
//! Sync Zoom with Other Pane gives the other panes the focused pane's zoom.
//! Design: docs/MENU_BAR.md, "Panes".

use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::schematic::SelectedItem;

use super::document_pane_actions::PaneOutcome;
use super::document_panes::TiledPanes;
use crate::app::Signex;
use crate::document_panes::{PaneCamera, SplitRefusal, zoom_kept_centre};

/// What moves between the main canvases and a pane canvas when the focus
/// moves to another pane.
pub(crate) struct PaneHandover {
    /// The document the main canvases drew, their view and, for a second
    /// view of the same sheet, their selection.
    leaving: Option<(PathBuf, PaneCamera, Option<Vec<SelectedItem>>)>,
    /// The view of the pane now focused, read from its own canvas, and its
    /// selection for a second view of the same sheet.
    arriving: Option<(PaneCamera, Option<Vec<SelectedItem>>)>,
    /// The document of the pane now focused.
    focused: PathBuf,
}

impl Signex {
    /// The view of `path` on the main canvases: the sheet canvas for a
    /// sheet, the board canvas for a board, none for anything else.
    fn main_camera_of(&self, path: &Path) -> Option<PaneCamera> {
        if self.document_state.engines.contains_key(path) {
            return Some(self.interaction_state.canvas.live_camera());
        }
        self.document_state
            .pcb_board(path)
            .map(|_| self.interaction_state.pcb_canvas.live_camera())
    }

    /// The view of the unfocused pane canvas of `path`, and its selection
    /// when it is a sheet.
    fn pane_camera_of(&self, path: &Path) -> Option<(PaneCamera, Option<Vec<SelectedItem>>)> {
        let panes = &self.interaction_state.tile_panes;
        if let Some(slot) = panes.schematic.get(path) {
            return Some((slot.live_camera(), Some(slot.selected.clone())));
        }
        panes
            .board
            .get(path)
            .map(|board| (board.live_camera(), None))
    }

    /// The document pane `n` of `view` draws.
    fn pane_document(&self, view: &TiledPanes, n: usize) -> Option<PathBuf> {
        let tab = self.document_state.tabs.get(*view.tabs.get(n)?)?;
        Some(tab.path.clone())
    }

    /// Read after the panes settled and before their canvases are
    /// refreshed: when the focus moved to another pane since the last
    /// settle, what the hand-over takes. Never inside a window's event,
    /// whose canvases are swapped into the main slots.
    pub(crate) fn pane_handover(&self) -> Option<PaneHandover> {
        if self.interaction_state.acting_window.is_some() {
            return None;
        }
        let view = self.tiled_pane_view()?;
        let focused = self.pane_document(&view, view.focused)?;
        let (was_pane, was_path) = self.ui_state.workspace_layouts.panes.focus.clone()?;
        // By id: an earlier pane that closed moved this one's place, not
        // the focus.
        if was_pane == view.focused_id && was_path == focused {
            return None;
        }
        let same = was_path == focused;
        let is_sheet = self.document_state.engines.contains_key(&was_path);
        let leaving = self.main_camera_of(&was_path).map(|camera| {
            let selected =
                (same && is_sheet).then(|| self.interaction_state.canvas.selected.clone());
            (was_path.clone(), camera, selected)
        });
        let arriving = self
            .pane_camera_of(&focused)
            .map(|(camera, selected)| (camera, selected.filter(|_| same)));
        Some(PaneHandover {
            leaving,
            arriving,
            focused,
        })
    }

    /// After the pane canvases were refreshed: `handover` is made, and the
    /// focused pane is noted for the next settle.
    pub(crate) fn finish_pane_handover(&mut self, handover: Option<PaneHandover>) {
        if let Some(handover) = handover {
            self.hand_over_pane_views(handover);
        }
        if self.interaction_state.acting_window.is_some() {
            return;
        }
        let focus = self.tiled_pane_view().and_then(|view| {
            let path = self.pane_document(&view, view.focused)?;
            Some((view.focused_id, path))
        });
        self.ui_state.workspace_layouts.panes.focus = focus;
    }

    /// The focused pane's document takes the view its pane had; the pane
    /// left behind keeps the view the main canvases had.
    fn hand_over_pane_views(&mut self, handover: PaneHandover) {
        let PaneHandover {
            leaving,
            arriving,
            focused,
        } = handover;
        let is_sheet = self.document_state.engines.contains_key(&focused);
        if let Some((camera, selected)) = arriving {
            if is_sheet {
                let canvas = &mut self.interaction_state.canvas;
                canvas.set_pane_camera(camera);
                if let Some(selected) = selected {
                    canvas.selected = selected;
                }
            } else if self.document_state.pcb_board(&focused).is_some() {
                self.interaction_state.pcb_canvas.set_pane_camera(camera);
            }
        }
        if let Some((path, camera, selected)) = leaving {
            let panes = &mut self.interaction_state.tile_panes;
            if let Some(slot) = panes.schematic.get_mut(&path) {
                slot.set_pane_camera(camera);
                if let Some(selected) = selected {
                    slot.selected = selected;
                    slot.clear_overlay_cache();
                }
            } else if let Some(board) = panes.board.get_mut(&path) {
                board.set_pane_camera(camera);
            }
        }
        // The Properties panel follows the selection of the focused pane.
        if is_sheet {
            self.update_selection_info();
        }
    }

    /// The zoom of pane `n` of `view` in percent; `None` for a pane that
    /// shows no sheet or board.
    pub(crate) fn pane_zoom_percent(&self, view: &TiledPanes, n: usize) -> Option<f64> {
        let path = self.pane_document(view, n)?;
        if n == view.focused {
            if self.document_state.engines.contains_key(&path) {
                return Some(self.interaction_state.canvas.zoom_percent());
            }
            return self
                .document_state
                .pcb_board(&path)
                .map(|_| self.interaction_state.pcb_canvas.zoom_percent());
        }
        let panes = &self.interaction_state.tile_panes;
        panes
            .schematic
            .get(&path)
            .map(|slot| slot.zoom_percent())
            .or_else(|| panes.board.get(&path).map(|board| board.zoom_percent()))
    }

    /// Sync Zoom with Other Pane: every other pane that shows a sheet or a
    /// board takes the focused pane's zoom about its own centre.
    pub(crate) fn sync_pane_zoom(&mut self) -> PaneOutcome {
        let view = self.tiled_pane_view().ok_or(SplitRefusal::NotTiled)?;
        let focused = self
            .pane_document(&view, view.focused)
            .ok_or(SplitRefusal::NotTiled)?;
        let (_, _, scale) = self.main_camera_of(&focused).ok_or(SplitRefusal::NoZoom)?;
        let others: Vec<PathBuf> = (0..view.tabs.len())
            .filter(|n| *n != view.focused)
            .filter_map(|n| self.pane_document(&view, n))
            .collect();
        let panes = &mut self.interaction_state.tile_panes;
        for path in others {
            if let Some(slot) = panes.schematic.get_mut(&path) {
                let size = slot.viewport_size();
                let camera = zoom_kept_centre(slot.live_camera(), (size.width, size.height), scale);
                slot.set_pane_camera(camera);
            } else if let Some(board) = panes.board.get_mut(&path) {
                let size = board.viewport_size();
                let camera =
                    zoom_kept_centre(board.live_camera(), (size.width, size.height), scale);
                board.set_pane_camera(camera);
            }
        }
        Ok(Task::none())
    }
}
