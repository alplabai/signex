//! Start screen project rows (#2305): pin and unpin a project, open a pinned
//! one, and the row's right-click menu.

use std::path::{Path, PathBuf};

use signex_i18n::{ids, t};

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
            StartScreenMsg::RecentProjectClicked(path) => {
                self.interaction_state.start_row_menu = None;
                self.handle_open_recent_project(path)
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
            StartScreenMsg::PinsChecked(Ok(missing)) => {
                for path in &missing {
                    self.ui_state.pinned_projects.set_missing(path, true);
                }
                iced::Task::none()
            }
            StartScreenMsg::PinsChecked(Err(reason)) => {
                crate::diagnostics::log_warning(
                    t!(self.ui_state.i18n, ids::START_PIN_CHECK_FAILED, "reason" => reason)
                        .into_string(),
                );
                iced::Task::none()
            }
        }
    }

    /// Check which pins are missing on disk on a blocking worker; the
    /// answer comes back as [`StartScreenMsg::PinsChecked`]. Boot runs it
    /// once, so no pin is stat-ed on the UI thread.
    pub fn check_pinned_projects(&self) -> iced::Task<Message> {
        let paths = self.ui_state.pinned_projects.entries().to_vec();
        if paths.is_empty() {
            return iced::Task::none();
        }
        iced::Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                crate::pinned_projects::missing_files(paths)
            }),
            |result| crate::start_screen::message(StartScreenMsg::PinsChecked(result)),
        )
    }

    /// Open a pinned project through the File ▸ Open path. One whose file
    /// is gone stays pinned, is shown as missing with Remove, and a warning
    /// goes to Messages; any other refusal is reported as File ▸ Open
    /// reports it. One that opens is shown as present again.
    fn open_pinned_project(&mut self, path: PathBuf) -> iced::Task<Message> {
        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;
        match self.open_document_path(path.clone()) {
            Ok(task) => {
                self.ui_state.pinned_projects.set_missing(&path, false);
                task
            }
            Err(error) if is_not_found(&error) => {
                self.ui_state.pinned_projects.set_missing(&path, true);
                crate::diagnostics::log_warning(
                    t!(
                        self.ui_state.i18n,
                        ids::START_PIN_MISSING_WARNING,
                        "path" => path.display().to_string()
                    )
                    .into_string(),
                );
                iced::Task::none()
            }
            Err(error) => {
                self.report_refused_open(&error);
                iced::Task::none()
            }
        }
    }

    /// Clear a row menu left open after the Start screen went away (a
    /// project or tab opened by another route), so it never comes back or
    /// takes an Esc. Run from `finish_update`.
    pub(crate) fn drop_start_row_menu_off_screen(&mut self) {
        if !self.start_screen_shown() {
            self.interaction_state.start_row_menu = None;
        }
    }

    /// The Start screen is what the main window's centre shows: no project
    /// is active and no tab is open.
    pub(crate) fn start_screen_shown(&self) -> bool {
        self.document_state.active_project.is_none() && self.document_state.tabs.is_empty()
    }

    /// A project was renamed or moved: its pin follows it, in place. The new
    /// path is spelled as the recent list spells it (canonical), because the
    /// recent list re-spells it the same way in this handler; pinning it
    /// as given would let the two spellings differ and show it twice.
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

/// Whether an open failed because the file is not there.
fn is_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
            || matches!(
                cause.downcast_ref::<signex_types::project::ProjectError>(),
                Some(signex_types::project::ProjectError::NotFound(_))
            )
    })
}
