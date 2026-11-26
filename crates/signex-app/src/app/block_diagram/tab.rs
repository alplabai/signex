//! The block diagram as a document tab: one tab per open diagram, on the
//! main strip, titled by its file. The tab's path is the diagram's, which
//! Save writes and the dirty mark follows. It draws in the main window
//! only (`TabKind::is_block_diagram`). Closing the tab of a diagram with
//! unsaved edits keeps the diagram held, so Save All and the quit prompt
//! still see it and reopening the file shows those edits.

use std::path::{Path, PathBuf};

use iced::Task;

use crate::app::{Message, Signex, TabInfo, TabKind};

fn title_of(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

impl Signex {
    /// The index of the tab showing the diagram at `path`.
    pub(crate) fn block_diagram_tab(&self, path: &Path) -> Option<usize> {
        self.document_state
            .tabs
            .iter()
            .position(|tab| matches!(&tab.kind, TabKind::BlockDiagram(p) if p == path))
    }

    /// Focus the diagram's tab, opening it at the end of the strip when
    /// there is none.
    pub(crate) fn show_block_diagram_tab(&mut self, path: &Path) {
        let existing = self.block_diagram_tab(path);
        let idx = match existing {
            Some(idx) => idx,
            None => {
                let project_id = self.document_state.project_for_path(path).map(|p| p.id);
                self.document_state.tabs.push(TabInfo {
                    title: title_of(path),
                    path: path.to_path_buf(),
                    project_id,
                    kind: TabKind::BlockDiagram(path.to_path_buf()),
                });
                self.document_state.tabs.len() - 1
            }
        };
        if existing.is_none() || idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
    }

    /// Close the diagram tab at `idx`. A diagram with unsaved edits, or
    /// one being written, stays held; any other is let go.
    pub(crate) fn close_block_diagram_tab(
        &mut self,
        idx: usize,
        keep_session: bool,
    ) -> Task<Message> {
        let Some(TabKind::BlockDiagram(path)) = self
            .document_state
            .tabs
            .get(idx)
            .map(|tab| tab.kind.clone())
        else {
            return Task::none();
        };
        self.document_state.tabs.remove(idx);
        let diagrams = &mut self.document_state.block_diagrams;
        let keep = diagrams.saving.contains(&path)
            || diagrams.editors.get(&path).is_some_and(|e| e.is_dirty());
        if !keep {
            diagrams.editors.remove(&path);
        }
        let len = self.document_state.tabs.len();
        let active = self.document_state.active_tab;
        if active > 0 && (idx < active || active >= len) {
            self.document_state.active_tab = active - 1;
        }
        if keep_session {
            self.settle_document_tiling();
        } else {
            self.sync_active_tab();
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// The diagram the main window shows, when its active tab is one.
    pub(crate) fn block_diagram_tab_in_window(&self, is_main: bool) -> Option<PathBuf> {
        if !is_main {
            return None;
        }
        let docs = &self.document_state;
        match &docs.tabs.get(docs.active_tab)?.kind {
            TabKind::BlockDiagram(path) => Some(path.clone()),
            _ => None,
        }
    }
}
