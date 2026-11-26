//! The Gerber Viewer as a document tab: one tab at a time, on the main
//! strip, opened by the first path that needs it and focused again after.
//! The tab holds no document: its path is the package's first path (empty
//! with nothing open), which no save, undo or History reads, and it does
//! not undock (`TabKind::is_gerber_viewer`).

use std::path::PathBuf;

use iced::keyboard::{self, Key};
use signex_i18n::{ids, t};

use super::GerberViewerMsg;
use crate::app::{Message, Signex, TabInfo, TabKind};

/// The Open menu's key on the tab (mockup row 193, "Keyboard").
const OPEN_KEY: &str = "o";

impl Signex {
    /// The index of the Gerber Viewer tab, if one is open.
    pub(crate) fn gerber_viewer_tab(&self) -> Option<usize> {
        self.document_state
            .tabs
            .iter()
            .position(|tab| tab.kind.is_gerber_viewer())
    }

    /// Focus the Gerber Viewer tab, opening it at the end of the strip when
    /// there is none.
    pub(crate) fn open_gerber_viewer_tab(&mut self) {
        let existing = self.gerber_viewer_tab();
        let idx = match existing {
            Some(idx) => idx,
            None => {
                let title = self.gerber_viewer_title();
                let path = self.gerber_viewer_tab_path();
                self.document_state.tabs.push(TabInfo {
                    title,
                    path,
                    project_id: None,
                    kind: TabKind::GerberViewer,
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

    /// The tab's title: the package's name, or "Gerber Viewer" with none.
    pub(crate) fn gerber_viewer_title(&self) -> String {
        self.document_state
            .gerber_viewer
            .open
            .as_ref()
            .and_then(|open| open.paths.first())
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| t!(self.ui_state.i18n, ids::GERBER_VIEWER_TAB_TITLE).into_string())
    }

    fn gerber_viewer_tab_path(&self) -> PathBuf {
        self.document_state
            .gerber_viewer
            .open
            .as_ref()
            .and_then(|open| open.paths.first().cloned())
            .unwrap_or_default()
    }

    /// Give the tab the title and path of what it shows now.
    pub(super) fn retitle_gerber_viewer_tab(&mut self) {
        let Some(idx) = self.gerber_viewer_tab() else {
            return;
        };
        let title = self.gerber_viewer_title();
        let path = self.gerber_viewer_tab_path();
        let tab = &mut self.document_state.tabs[idx];
        tab.title = title;
        tab.path = path;
    }

    /// Close the Gerber Viewer tab: the package goes, the recent list stays
    /// and is checked again, so a package moved meanwhile reads "Files not
    /// found" the next time it shows.
    pub(crate) fn close_gerber_viewer_tab(
        &mut self,
        idx: usize,
        keep_session: bool,
    ) -> iced::Task<Message> {
        self.document_state.tabs.remove(idx);
        let viewer = &mut self.document_state.gerber_viewer;
        viewer.open = None;
        viewer.reading = None;
        viewer.open_menu = false;
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
        self.check_recent_gerber_packages()
    }

    /// Whether the main window shows the Gerber Viewer tab.
    pub(crate) fn gerber_viewer_tab_in_window(&self, is_main: bool) -> bool {
        let docs = &self.document_state;
        is_main
            && docs
                .tabs
                .get(docs.active_tab)
                .is_some_and(|tab| tab.kind.is_gerber_viewer())
    }

    /// Ctrl+O (Cmd+O) on the tab opens its Open menu, as on the empty state
    /// (mockup rows 193 and 193f), in place of File ▸ Open. Esc closes the
    /// menu, lets go of a held row or clears the selection (mockup row
    /// 193, "Keyboard"); with none of those it passes on.
    pub(crate) fn gerber_viewer_key(
        &self,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Option<Message> {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        let shown = self
            .document_state
            .tab_shown_in_window(window, &self.ui_state)?;
        if !shown.kind.is_gerber_viewer() {
            return None;
        }
        if matches!(key.as_ref(), Key::Named(keyboard::key::Named::Escape)) && modifiers.is_empty()
        {
            // An overlay over the tab takes Esc first.
            let ours = self.document_state.gerber_viewer.has_escape()
                && self.escape_overlay_message().is_none();
            return ours.then(|| Message::from(GerberViewerMsg::EscapePressed));
        }
        let is_open_key = matches!(key.as_ref(), Key::Character(c) if c.eq_ignore_ascii_case(OPEN_KEY))
            && modifiers.command()
            && !modifiers.shift()
            && !modifiers.alt();
        is_open_key.then(|| Message::from(GerberViewerMsg::OpenMenuToggled))
    }
}
