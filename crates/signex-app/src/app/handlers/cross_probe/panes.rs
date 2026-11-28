//! Cross probe across the panes of the document area (mockup row 207): what
//! the focused pane has selected is outlined in every other pane on the
//! other side. A part selected on the sheet outlines its footprint on a
//! board pane, a footprint selected on the board outlines its symbol on a
//! sheet pane, and a net its members. It follows Cross Select Mode, and it
//! is drawn again only when the selection, the panes or a board changed.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::app::Signex;
use crate::app::state::cross_select::CrossSelection;
use crate::pcb_canvas::{Highlight, PcbSelectionState};

use super::surfaces::board_items_for;

impl Signex {
    /// The documents the unfocused panes draw, in pane order.
    fn unfocused_pane_documents(&self) -> Vec<PathBuf> {
        let Some(view) = self.tiled_pane_view() else {
            return Vec::new();
        };
        view.tabs
            .iter()
            .enumerate()
            .filter(|(n, _)| *n != view.focused)
            .filter_map(|(_, &index)| self.document_state.tabs.get(index))
            .map(|tab| tab.path.clone())
            .collect()
    }

    /// Outline the focused pane's selection in the other panes, when it,
    /// the panes or a board they show changed since the last outline.
    pub(super) fn outline_counterparts_in_panes(&mut self) {
        if !self.interaction_state.cross_select.enabled {
            self.ui_state.workspace_layouts.panes.outlined = None;
            return;
        }
        let others = self.unfocused_pane_documents();
        if others.is_empty() {
            self.ui_state.workspace_layouts.panes.outlined = None;
            return;
        }
        let source = self
            .main_sheet()
            .map(|path| (path, true))
            .or_else(|| self.main_board().map(|path| (path, false)));
        let Some((source, is_sheet)) = source else {
            return;
        };
        let key = self.outline_key(&source, is_sheet, &others);
        if self.ui_state.workspace_layouts.panes.outlined == Some(key) {
            return;
        }
        self.ui_state.workspace_layouts.panes.outlined = Some(key);
        if is_sheet {
            let selected = self.interaction_state.canvas.selected.clone();
            let what = self.cross_selection_from_sheet(&source, &selected);
            for path in others {
                let outline = self.board_outline(&path, &what);
                if let Some(board) = self.interaction_state.tile_panes.board.get_mut(&path)
                    && board.highlight() != &outline
                {
                    board.set_highlight(outline);
                }
            }
        } else {
            let items = self.board_selection(None, &source);
            let what = self.cross_selection_from_board(&source, &items);
            for path in others {
                let outline = self.sheet_items_for(&path, &what);
                if let Some(slot) = self.interaction_state.tile_panes.schematic.get_mut(&path)
                    && slot.selected != outline
                {
                    slot.selected = outline;
                    slot.clear_overlay_cache();
                }
            }
        }
    }

    /// A fingerprint of what an outline depends on: the focused document
    /// and its selection, the other panes' documents and their boards'
    /// revisions.
    fn outline_key(&self, source: &Path, is_sheet: bool, others: &[PathBuf]) -> u64 {
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        if is_sheet {
            self.interaction_state.canvas.selected.hash(&mut hasher);
        } else {
            self.board_selection(None, source).hash(&mut hasher);
        }
        for path in others {
            path.hash(&mut hasher);
            self.document_state
                .pcb_revisions
                .get(path)
                .hash(&mut hasher);
            let has_canvas = self.interaction_state.tile_panes.board.contains_key(path)
                || self
                    .interaction_state
                    .tile_panes
                    .schematic
                    .contains_key(path);
            has_canvas.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// The outline of what `what` names on the board at `path`: its
    /// footprints and the copper of its nets, painted as a selection.
    fn board_outline(&self, path: &Path, what: &CrossSelection) -> Highlight {
        let Some(board) = self.document_state.pcb_board(path) else {
            return Highlight::default();
        };
        let items = board_items_for(board, what);
        if items.is_empty() {
            return Highlight::default();
        }
        let revision = self
            .document_state
            .pcb_revisions
            .get(path)
            .copied()
            .unwrap_or(0);
        let mut state = PcbSelectionState::default();
        state.sync(Some(path), revision);
        state.set_view(&self.pcb_layer_view(path));
        state.build_index(board);
        let picked = state.select_items(board, &items);
        // The focused pane's own selection said what was left out already.
        for line in state.take_report() {
            crate::diagnostics::log_debug(line);
        }
        if !picked.changed {
            return Highlight::default();
        }
        state.paint(board, None).0
    }
}
