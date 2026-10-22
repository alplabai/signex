//! File ▸ Open Recent handlers: record on a successful project open,
//! reopen an entry, clear the list.

use std::path::{Path, PathBuf};

use super::super::super::*;

impl Signex {
    /// Remember `path` as the most recent project and persist the list.
    /// Called only after a project actually loaded, so a failed open
    /// never enters the list.
    pub(super) fn record_recent_project(&mut self, path: &Path) {
        self.ui_state.recent_projects.record(path);
        self.persist_recent_projects();
    }

    fn persist_recent_projects(&self) {
        crate::fonts::write_recent_projects(self.ui_state.recent_projects.entries());
    }

    /// File ▸ Open Recent ▸ `<project>`. Reuses the File ▸ Open path for a
    /// project that still exists; a vanished one is dropped from the list
    /// and reported, and nothing is opened.
    pub(crate) fn handle_open_recent_project(&mut self, path: PathBuf) -> iced::Task<Message> {
        if path.is_file() {
            return self.handle_document_file_opened(Some(path));
        }
        if self.ui_state.recent_projects.remove(&path) {
            self.persist_recent_projects();
        }
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::open(format!(
            "{} no longer exists. It has been removed from File > Open Recent.",
            path.display()
        )));
        iced::Task::none()
    }

    /// File ▸ Open Recent ▸ Clear Recent.
    pub(crate) fn handle_clear_recent_projects(&mut self) -> iced::Task<Message> {
        self.ui_state.recent_projects.clear();
        self.persist_recent_projects();
        iced::Task::none()
    }
}
