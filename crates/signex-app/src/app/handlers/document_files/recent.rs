//! File ▸ Open Recent handlers: record on a successful project open,
//! reopen an entry, clear the list.

use std::path::{Path, PathBuf};

use super::super::super::*;

impl Signex {
    /// Remember `path` as the most recent project and persist the list.
    /// Called only after a project actually loaded, so a failed open
    /// never enters the list.
    pub(super) fn record_recent_project(&mut self, path: &Path) -> iced::Task<Message> {
        self.ui_state.recent_projects.record(path);
        self.persist_recent_projects()
    }

    /// A project was renamed or moved: its Open Recent entry (as the list
    /// spelled it, see `RecentProjects::spelling_of`) follows it, in place.
    pub(crate) fn replace_recent_project(&mut self, old: &Path, new: &Path) -> iced::Task<Message> {
        if self.ui_state.recent_projects.replace(old, new) {
            self.persist_recent_projects()
        } else {
            iced::Task::none()
        }
    }

    /// The `Task` that writes the list to `prefs.json` off the UI thread (#1351).
    fn persist_recent_projects(&self) -> iced::Task<Message> {
        let entries: Vec<_> = self.ui_state.recent_projects.entries().to_vec();
        crate::fonts::background_pref_write("recent_projects", move || {
            crate::fonts::write_recent_projects(&entries)
        })
    }

    /// File ▸ Open Recent ▸ `<project>`. Reuses the File ▸ Open path for a
    /// project that still exists; a vanished one is dropped from the list
    /// and reported, and nothing is opened.
    pub(crate) fn handle_open_recent_project(&mut self, path: PathBuf) -> iced::Task<Message> {
        if path.is_file() {
            return self.handle_document_file_opened(Some(path));
        }
        let persist = if self.ui_state.recent_projects.remove(&path) {
            self.persist_recent_projects()
        } else {
            iced::Task::none()
        };
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::open(format!(
            "{} no longer exists. It has been removed from File > Open Recent.",
            path.display()
        )));
        persist
    }

    /// File ▸ Open Recent ▸ Clear Recent.
    pub(crate) fn handle_clear_recent_projects(&mut self) -> iced::Task<Message> {
        self.ui_state.recent_projects.clear();
        self.persist_recent_projects()
    }
}
