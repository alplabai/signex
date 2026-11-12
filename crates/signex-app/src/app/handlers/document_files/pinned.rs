//! Start screen project rows (#2305): pin and unpin a project, open a pinned
//! one, and the row's right-click menu.

use std::path::{Path, PathBuf};

use super::super::super::*;
use crate::start_screen::{StartRowMenu, StartScreenMsg};

impl Signex {
    /// Route a Start screen row event. Exhaustive: a new event is a compile
    /// error until it is routed.
    pub(crate) fn dispatch_start_screen_message(
        &mut self,
        msg: StartScreenMsg,
    ) -> iced::Task<Message> {
        match msg {
            StartScreenMsg::ProjectPinned(path) => {
                self.interaction_state.start_row_menu = None;
                if self.ui_state.pinned_projects.pin(&path) {
                    self.persist_pinned_projects()
                } else {
                    iced::Task::none()
                }
            }
            StartScreenMsg::ProjectUnpinned(path) => {
                self.interaction_state.start_row_menu = None;
                if self.ui_state.pinned_projects.unpin(&path) {
                    self.persist_pinned_projects()
                } else {
                    iced::Task::none()
                }
            }
            StartScreenMsg::PinnedProjectClicked(path) => {
                self.interaction_state.start_row_menu = None;
                self.open_pinned_project(path)
            }
            StartScreenMsg::RowRightClicked(path) => {
                self.close_all_context_menus();
                let (x, y) = self.interaction_state.last_mouse_pos;
                self.interaction_state.start_row_menu = Some(StartRowMenu { path, x, y });
                iced::Task::none()
            }
            StartScreenMsg::RowMenuDismissed => {
                self.interaction_state.start_row_menu = None;
                iced::Task::none()
            }
        }
    }

    /// Open a pinned project through the File ▸ Open path. One that is no
    /// longer on disk stays pinned and is shown as missing, with Remove;
    /// one that came back is shown as present again.
    fn open_pinned_project(&mut self, path: PathBuf) -> iced::Task<Message> {
        let exists = path.is_file();
        self.ui_state.pinned_projects.set_missing(&path, !exists);
        if exists {
            self.handle_document_file_opened(Some(path))
        } else {
            iced::Task::none()
        }
    }

    /// A project was renamed or moved: its pin follows it, in place.
    pub(super) fn replace_pinned_project(&mut self, old: &Path, new: &Path) -> iced::Task<Message> {
        let new = crate::recent_projects::RecentProjects::spelling_of(new);
        if self.ui_state.pinned_projects.replace(old, &new) {
            self.persist_pinned_projects()
        } else {
            iced::Task::none()
        }
    }

    /// The `Task` that writes the pinned list to `prefs.json` off the UI
    /// thread, as the recent list is written (#1351).
    fn persist_pinned_projects(&self) -> iced::Task<Message> {
        let entries: Vec<_> = self.ui_state.pinned_projects.entries().to_vec();
        crate::fonts::background_pref_write("pinned_projects", move || {
            crate::fonts::write_pinned_projects(&entries)
        })
    }
}
