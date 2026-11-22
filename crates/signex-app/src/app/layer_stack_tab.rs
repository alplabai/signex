//! A board's Layer Stack as a page tab (mockup row 105, DECISIONS
//! 2026-11-18). One tab per board, next to its PCB tab. The tab's path is the
//! board's (docs/PROJECT_TAB.md, "Page tabs"), so it shows the board's dirty
//! dot and Save writes the board.
//!
//! The tab holds no document. Lookups by path take the first tab, so the Layer
//! Stack tab always sits after its board's: every path lookup then finds the
//! board. Opening, dragging and closing keep that order.

use std::path::{Path, PathBuf};

use iced::{Element, Widget as _};
use signex_i18n::ids;

use crate::app::state::{ProjectId, WindowKind};
use crate::app::{Message, Signex, TabInfo, TabKind, page_tab};
use crate::dock::DockMessage;

/// The index of the tab of `kind` for `board`.
fn tab_at(tabs: &[TabInfo], board: &Path, is_page: bool) -> Option<usize> {
    tabs.iter().position(|tab| {
        tab.path == board
            && if is_page {
                tab.kind.is_layer_stack()
            } else {
                matches!(tab.kind, TabKind::Pcb)
            }
    })
}

/// Move any Layer Stack tab left of its board to just after it, and return
/// where the tab at `active` went.
pub(crate) fn keep_pages_after_boards(tabs: &mut Vec<TabInfo>, mut active: usize) -> usize {
    let pages: Vec<PathBuf> = tabs
        .iter()
        .filter(|tab| tab.kind.is_layer_stack())
        .map(|tab| tab.path.clone())
        .collect();
    for board in pages {
        let (Some(page), Some(owner)) = (tab_at(tabs, &board, true), tab_at(tabs, &board, false))
        else {
            continue;
        };
        if page > owner {
            continue;
        }
        let tab = tabs.remove(page);
        // The board moved one left, so its old index is just after it.
        tabs.insert(owner, tab);
        active = if active == page {
            owner
        } else if active > page && active <= owner {
            active - 1
        } else {
            active
        };
    }
    active
}

impl Signex {
    /// The board the Layer Stack command acts on: an undocked window's own
    /// board, else the active tab's.
    fn layer_stack_board(&self) -> Option<PathBuf> {
        let docs = &self.document_state;
        if let Some(window) = self.interaction_state.acting_window
            && let Some(WindowKind::UndockedTab { path, .. }) = self.ui_state.windows.get(&window)
            && docs.has_pcb_tab(path)
        {
            return Some(path.clone());
        }
        let tab = docs.tabs.get(docs.active_tab)?;
        matches!(tab.kind, TabKind::Pcb | TabKind::LayerStack).then(|| tab.path.clone())
    }

    /// `<project> — Layer Stack`; a board with no project goes by its file
    /// name.
    pub(crate) fn layer_stack_title(&self, board: &Path, project: Option<ProjectId>) -> String {
        let name = project
            .and_then(|id| self.document_state.project_by_id(id))
            .map(page_tab::project_display_name)
            .or_else(|| {
                board
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        page_tab::page_title(&self.ui_state.i18n, &name, ids::PCB_STACK_TITLE)
    }

    /// Focus the Layer Stack tab of the board in view, or open it right after
    /// the board's tab. Every Layer Stack entry point lands here.
    pub(crate) fn open_layer_stack_tab(&mut self) {
        let Some(board) = self.layer_stack_board() else {
            return;
        };
        let tabs = &self.document_state.tabs;
        let Some(owner) = tab_at(tabs, &board, false) else {
            return;
        };
        let idx = match tab_at(tabs, &board, true) {
            Some(idx) => idx,
            None => {
                let project_id = tabs[owner].project_id;
                let title = self.layer_stack_title(&board, project_id);
                let at = owner + 1;
                self.document_state.tabs.insert(
                    at,
                    TabInfo {
                        title,
                        path: board,
                        cached_document: None,
                        project_id,
                        kind: TabKind::LayerStack,
                    },
                );
                // The tab went in before any tab after the board.
                if self.document_state.active_tab >= at {
                    self.document_state.active_tab += 1;
                }
                at
            }
        };
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
    }

    /// Close a Layer Stack tab. The board, its window and its unsaved work
    /// are not touched.
    pub(crate) fn close_layer_stack_tab(&mut self, idx: usize, keep_session: bool) {
        self.document_state.tabs.remove(idx);
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
    }

    /// Drop `board`'s Layer Stack tab while its PCB tab closes. The tab sits
    /// after the board's, so the board's index stays valid.
    pub(crate) fn drop_layer_stack_tab_of(&mut self, board: &Path) {
        let docs = &mut self.document_state;
        let Some(page) = tab_at(&docs.tabs, board, true) else {
            return;
        };
        docs.tabs.remove(page);
        if docs.active_tab > page {
            docs.active_tab -= 1;
        }
        docs.active_tab = docs.active_tab.min(docs.tabs.len().saturating_sub(1));
    }

    /// Whether the main window shows a Layer Stack tab.
    pub(crate) fn layer_stack_tab_in_window(&self, is_main: bool) -> bool {
        let docs = &self.document_state;
        is_main
            && docs
                .tabs
                .get(docs.active_tab)
                .is_some_and(|tab| tab.kind.is_layer_stack())
    }

    pub(crate) fn view_layer_stack_tab(&self) -> Element<'_, Message> {
        crate::panels::layer_stack::view(&self.document_state.panel_ctx)
            .map(|msg| Message::Dock(DockMessage::Panel(msg)))
            .boxed()
    }
}

#[cfg(test)]
#[path = "layer_stack_tab_tests.rs"]
mod tests;
