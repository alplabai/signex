//! Window ▸ Tile in the main window (#2249): the docked documents in panes,
//! side by side (Tile Vertically) or stacked (Tile Horizontally), the
//! direction the undocked windows are arranged in (#2247). The focused pane
//! is the active tab and draws on the main canvases; every other pane draws
//! its document on a canvas of its own and makes it active on a press.
//! Untile, or closing down to one document, goes back to one pane. See
//! docs/MENU_BAR.md#window-tile.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::Task;

use crate::app::{Message, Signex, TileLayout};

impl Signex {
    /// The tabs the main window shows, by index in tab-strip order: every tab
    /// that is not in a window of its own.
    #[must_use]
    pub fn docked_tab_indices(&self) -> Vec<usize> {
        let elsewhere: HashSet<usize> = self
            .ui_state
            .windows
            .values()
            .filter_map(|kind| self.document_state.tab_index_of_window_kind(kind).flatten())
            .collect();
        (0..self.document_state.tabs.len())
            .filter(|index| !elsewhere.contains(index))
            .collect()
    }

    /// Two documents or more can share the main window in panes.
    pub(crate) fn can_tile_documents(&self) -> bool {
        self.docked_tab_indices().len() >= 2
    }

    /// The layout and the tab of each pane when the main window is tiled:
    /// two docked tabs or more, the active one among them.
    #[must_use]
    pub fn tiled_panes(&self) -> Option<(TileLayout, Vec<usize>)> {
        let layout = self.ui_state.document_tiling?;
        let panes = self.docked_tab_indices();
        (panes.len() >= 2 && panes.contains(&self.document_state.active_tab))
            .then_some((layout, panes))
    }

    /// Window ▸ Tile: the docked documents into panes, and the main window
    /// with the undocked document windows over its screen. Either alone is
    /// done when the other has nothing to arrange.
    pub(crate) fn tile(&mut self, layout: TileLayout) -> Task<Message> {
        let panes = self.tile_documents(layout);
        if self.tile_plan(layout).is_some() {
            return self.tile_windows(layout);
        }
        if !panes {
            crate::diagnostics::log_warning(
                "Tile: needs two documents in the main window, or an undocked document window \
                 and a main window whose screen is known; nothing was arranged.",
            );
        }
        Task::none()
    }

    /// Show the docked documents in `layout` panes; `false` with fewer than
    /// two.
    fn tile_documents(&mut self, layout: TileLayout) -> bool {
        let count = self.docked_tab_indices().len();
        if count < 2 {
            return false;
        }
        let before = self.canvas_bounds();
        self.ui_state.document_tiling = Some(layout);
        self.refresh_tile_pane_canvases();
        self.follow_main_canvas_resize(before);
        crate::diagnostics::log_info(format!(
            "Tile: {count} documents {} in the main window",
            match layout {
                TileLayout::SideBySide => "side by side",
                TileLayout::Stacked => "one above the other",
            }
        ));
        true
    }

    /// Window ▸ Untile: the active document fills the main window again.
    pub(crate) fn untile_documents(&mut self) -> Task<Message> {
        if self.ui_state.document_tiling.is_none() {
            return Task::none();
        }
        let before = self.canvas_bounds();
        self.ui_state.document_tiling = None;
        self.interaction_state.tile_panes = Default::default();
        self.follow_main_canvas_resize(before);
        Task::none()
    }

    /// A press on the pane of tab `index`: its document becomes the active
    /// one, as a tab click would make it.
    pub(crate) fn focus_tile_pane(&mut self, index: usize) -> Task<Message> {
        let Some((_, panes)) = self.tiled_panes() else {
            return Task::none();
        };
        if index == self.document_state.active_tab || !panes.contains(&index) {
            return Task::none();
        }
        let before = self.canvas_bounds();
        self.park_active_schematic_session();
        self.document_state.active_tab = index;
        self.sync_active_tab();
        self.follow_main_canvas_resize(before);
        Task::none()
    }

    /// After a tab closed, undocked or came back: fewer than two docked
    /// documents end the tiling, and every unfocused pane gets its canvas.
    pub(crate) fn settle_document_tiling(&mut self) {
        if self.ui_state.document_tiling.is_some() && !self.can_tile_documents() {
            self.ui_state.document_tiling = None;
        }
        self.refresh_tile_pane_canvases();
    }

    /// A canvas for each unfocused schematic or board pane, kept while the
    /// pane stays unfocused; the focused pane's and the gone panes' dropped.
    /// A pane that was focused gets a fresh one from its document as it is
    /// now.
    pub(crate) fn refresh_tile_pane_canvases(&mut self) {
        let Some((_, panes)) = self.tiled_panes() else {
            self.interaction_state.tile_panes = Default::default();
            return;
        };
        let active = self.document_state.active_tab;
        let shown: Vec<PathBuf> = panes
            .into_iter()
            .filter(|index| *index != active)
            .filter_map(|index| self.document_state.tabs.get(index))
            .map(|tab| tab.path.clone())
            .collect();
        let tile_panes = &mut self.interaction_state.tile_panes;
        tile_panes.schematic.retain(|path, _| shown.contains(path));
        tile_panes.board.retain(|path, _| shown.contains(path));
        for path in shown {
            let missing_sheet = !self
                .interaction_state
                .tile_panes
                .schematic
                .contains_key(&path);
            if missing_sheet && let Some(engine) = self.document_state.engines.get(&path) {
                let size =
                    crate::app::projector::effective_paper_size(&engine.document().paper_size);
                let paper = crate::panels::paper_dimensions(&size);
                let slot = self.seeded_canvas_slot(&path, paper);
                self.interaction_state
                    .tile_panes
                    .schematic
                    .insert(path.clone(), slot);
            }
            if !self.interaction_state.tile_panes.board.contains_key(&path)
                && let Some(board) = self.new_pcb_canvas_for(&path)
            {
                self.interaction_state.tile_panes.board.insert(path, board);
            }
        }
    }

    /// A schematic canvas of its own for the sheet at `path` on a `paper`
    /// (width, height) mm sheet: its document drawn from the first frame,
    /// fitted to the sheet, with fresh pan, zoom and selection. An undocked
    /// window and an unfocused tile pane each draw on one.
    pub(crate) fn seeded_canvas_slot(
        &self,
        path: &Path,
        paper: (f32, f32),
    ) -> crate::canvas::CanvasSlot {
        let mut slot = crate::canvas::CanvasSlot::new();
        if let Some(engine) = self.document_state.engines.get(path) {
            slot.set_render_cache(Some(
                crate::schematic_runtime::SchematicRenderCache::from_sheet_at(
                    engine.document(),
                    engine.revision(),
                ),
            ));
        }
        // Theme, snap and grid are read from `UiState` in `view` (#631); the
        // paper, frame, special strings and designators are the document's.
        slot.paper_width_mm = paper.0;
        slot.paper_height_mm = paper.1;
        slot.sheet_frame = self.sheet_frame_for_path(path, paper.0, paper.1);
        slot.special_strings = self.special_strings_for_path(path);
        slot.designators = self.designators_for_path(path);
        slot.fit_to_sheet();
        slot
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app::handlers::menu::export::tests as fx;
    use crate::app::{TabInfo, TabKind, TileLayout};
    use signex_engine::DocumentPatch;

    /// A change to the sheet of an unfocused pane redraws that pane, as it
    /// redraws an undocked window on the sheet.
    #[test]
    fn a_change_to_an_unfocused_sheet_redraws_its_pane() {
        let dir = "/w/tile-panes";
        let mut app = fx::app_workspace(dir, &["left.snxsch", "right.snxsch"]);
        let paths = ["left.snxsch", "right.snxsch"].map(|name| PathBuf::from(dir).join(name));
        for path in &paths {
            fx::open_with(
                &mut app.document_state,
                path,
                fx::sheet_with_net("R1", "N1", &[]),
            );
            app.document_state.tabs.push(TabInfo {
                title: path.display().to_string(),
                path: path.clone(),
                cached_document: None,
                project_id: None,
                kind: TabKind::Schematic,
            });
        }
        app.document_state.active_tab = 1;
        app.document_state.active_path = Some(paths[1].clone());
        let _task = app.tile(TileLayout::SideBySide);
        let clears = app.interaction_state.tile_panes.schematic[&paths[0]].content_cache_clears;

        app.after_sheet_change(&paths[0], DocumentPatch::NET_COLORS);

        let pane = &app.interaction_state.tile_panes.schematic[&paths[0]];
        assert!(pane.content_cache_clears > clears, "the pane redraws");
    }
}
