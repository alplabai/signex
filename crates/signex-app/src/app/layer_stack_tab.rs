//! A board's Layer Stack as a page tab (mockup row 105, DECISIONS
//! 2026-11-18), and the rules every page of a board keeps (the Polygon
//! Manager, mockup row 178, is the other). One tab of each kind per board,
//! next to its PCB tab. The tab's path is the board's (docs/PROJECT_TAB.md,
//! "Page tabs"), so it shows the board's dirty dot and Save writes the board.
//!
//! A page tab holds no document. Lookups by path take the first tab, so a
//! page always sits after its board's: every path lookup then finds the
//! board. Opening, dragging and closing keep that order.

use std::path::{Path, PathBuf};

use iced::{Element, Widget as _};
use signex_i18n::MessageId;

use crate::app::state::{ProjectId, WindowKind};
use crate::app::{Message, Signex, TabInfo, TabKind, page_tab};
use crate::dock::DockMessage;

/// Whether `kind` is the same kind of tab as `page`.
fn same_kind(kind: &TabKind, page: &TabKind) -> bool {
    std::mem::discriminant(kind) == std::mem::discriminant(page)
}

/// The index of `board`'s tab of the kind `page`, or of its PCB tab when
/// `page` is `None`.
fn tab_at(tabs: &[TabInfo], board: &Path, page: Option<&TabKind>) -> Option<usize> {
    tabs.iter().position(|tab| {
        tab.path == board
            && match page {
                Some(page) => same_kind(&tab.kind, page),
                None => matches!(tab.kind, TabKind::Pcb),
            }
    })
}

/// Move any board page tab left of its board to just after it, and return
/// where the tab at `active` went.
pub(crate) fn keep_pages_after_boards(tabs: &mut Vec<TabInfo>, active: usize) -> usize {
    // One pass per page kind: `keep_pages_after_owners` finds one page of
    // its kind per path.
    let is_board: fn(&TabKind) -> bool = |kind| matches!(kind, TabKind::Pcb);
    let active = keep_pages_after_owners(tabs, active, TabKind::is_layer_stack, is_board);
    let active = keep_pages_after_owners(tabs, active, TabKind::is_polygon_manager, is_board);
    keep_pages_after_owners(tabs, active, TabKind::is_length_matching, is_board)
}

/// Move any page tab (`is_page`) left of its owner's tab (`is_owner`, the
/// same path) to just after it, and return where the tab at `active` went.
/// The board pages and the Pin table tab all keep their place this way.
pub(crate) fn keep_pages_after_owners(
    tabs: &mut Vec<TabInfo>,
    mut active: usize,
    is_page: fn(&TabKind) -> bool,
    is_owner: fn(&TabKind) -> bool,
) -> usize {
    let at = |tabs: &[TabInfo], path: &Path, kind: fn(&TabKind) -> bool| {
        tabs.iter()
            .position(|tab| tab.path == path && kind(&tab.kind))
    };
    let pages: Vec<PathBuf> = tabs
        .iter()
        .filter(|tab| is_page(&tab.kind))
        .map(|tab| tab.path.clone())
        .collect();
    for path in pages {
        let (Some(page), Some(owner)) = (
            at(tabs.as_slice(), &path, is_page),
            at(tabs.as_slice(), &path, is_owner),
        ) else {
            continue;
        };
        if page > owner {
            continue;
        }
        let tab = tabs.remove(page);
        // The owner moved one left, so its old index is just after it.
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
    /// The board a page command acts on: an undocked window's own board,
    /// else the active tab's (its PCB tab or one of its pages).
    pub(crate) fn page_board(&self) -> Option<PathBuf> {
        let docs = &self.document_state;
        if let Some(window) = self.interaction_state.acting_window
            && let Some(WindowKind::UndockedTab { path, .. }) = self.ui_state.windows.get(&window)
            && docs.has_pcb_tab(path)
        {
            return Some(path.clone());
        }
        let tab = docs.tabs.get(docs.active_tab)?;
        (matches!(tab.kind, TabKind::Pcb) || tab.kind.is_board_page()).then(|| tab.path.clone())
    }

    /// `<project> — <page>` for a page of `board`; a board with no project
    /// goes by its file name.
    pub(crate) fn board_page_title(
        &self,
        board: &Path,
        project: Option<ProjectId>,
        page: MessageId,
    ) -> String {
        let name = project
            .and_then(|id| self.document_state.project_by_id(id))
            .map(page_tab::project_display_name)
            .or_else(|| {
                board
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        page_tab::page_title(&self.ui_state.i18n, &name, page)
    }

    /// Focus the Layer Stack tab of the board in view, or open it right after
    /// the board's tab. Every Layer Stack entry point lands here.
    pub(crate) fn open_layer_stack_tab(&mut self) {
        self.open_board_page_tab(TabKind::LayerStack);
    }

    /// Focus the `page` tab of the board in view, or open it right after the
    /// board's tab. `None` without a board.
    pub(crate) fn open_board_page_tab(&mut self, page: TabKind) -> Option<PathBuf> {
        let board = self.page_board()?;
        let idx = self.ensure_board_page_tab(board.clone(), page)?;
        self.focus_board_page_tab(idx);
        Some(board)
    }

    /// `board`'s `page` tab, added right after the board's when missing.
    /// `None` when the board has no tab.
    fn ensure_board_page_tab(&mut self, board: PathBuf, page: TabKind) -> Option<usize> {
        let tabs = &self.document_state.tabs;
        let owner = tab_at(tabs, &board, None)?;
        if let Some(idx) = tab_at(tabs, &board, Some(&page)) {
            return Some(idx);
        }
        let project_id = tabs[owner].project_id;
        let title = self.board_page_title(&board, project_id, page_tab::page_name(&page)?);
        let at = owner + 1;
        self.document_state.tabs.insert(
            at,
            TabInfo {
                title,
                path: board,
                project_id,
                kind: page,
            },
        );
        // The tab went in before any tab after the board.
        if self.document_state.active_tab >= at {
            self.document_state.active_tab += 1;
        }
        Some(at)
    }

    fn focus_board_page_tab(&mut self, idx: usize) {
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
    }

    /// Bring back a Layer Stack tab a branch switch's reopen closed, once its
    /// board has a tab again.
    pub(crate) fn restore_layer_stack_tab(&mut self, board: PathBuf, takes_focus: bool) {
        if let Some(idx) = self.ensure_board_page_tab(board, TabKind::LayerStack)
            && takes_focus
        {
            self.focus_board_page_tab(idx);
        }
    }

    /// Close a document page tab (a board page or a Pin table). The board or
    /// symbol, its window and its unsaved work are not touched.
    pub(crate) fn close_document_page_tab(&mut self, idx: usize, keep_session: bool) {
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

    /// Drop `board`'s page tabs while its PCB tab closes. The pages sit after
    /// the board's, so the board's index stays valid.
    pub(crate) fn drop_board_pages_of(&mut self, board: &Path) {
        let docs = &mut self.document_state;
        while let Some(page) = docs
            .tabs
            .iter()
            .position(|tab| tab.path == board && tab.kind.is_board_page())
        {
            docs.tabs.remove(page);
            if docs.active_tab > page {
                docs.active_tab -= 1;
            }
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
        crate::panels::layer_stack::view(&self.document_view.panel_ctx)
            .map(|msg| Message::Dock(DockMessage::Panel(msg)))
            .boxed()
    }
}

#[cfg(test)]
#[path = "layer_stack_tab_tests.rs"]
mod tests;
