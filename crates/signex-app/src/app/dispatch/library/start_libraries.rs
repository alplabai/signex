//! The Start screen's Libraries page actions: the off-thread read of each
//! library's version control and Enable Version Control from its row
//! (#2591), and Remove from Signex (#2592). Design:
//! `docs/architecture/start-screen-libraries.md`.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::state::{ErrorNotice, NoticeAction};
use crate::panels::components_panel::global_prefs::{self, GlobalLibraryEntry};
use crate::start_screen::{
    StartPage, StartRowKind, StartRowMenu, StartScreenMsg, is_removable, library_folder, message,
};
use signex_library::library_vcs::{LibraryVcsState, ToolsOnPath, read_library_vcs_state};

impl Signex {
    /// Read the version control of every mounted library but Stock on a
    /// blocking worker; the answer lands as [`StartScreenMsg::LibraryVcsRead`].
    /// Run when the Libraries page opens.
    pub(crate) fn read_start_library_vcs(&mut self) -> Task<Message> {
        let mut dirs: Vec<PathBuf> = self
            .library
            .open_libraries
            .iter()
            .filter(|lib| !self.library.is_stock_root(&lib.root))
            .filter_map(|lib| library_folder(&lib.root).map(Path::to_path_buf))
            .collect();
        dirs.sort();
        dirs.dedup();
        let state = &mut self.ui_state.start_screen;
        state.library_vcs_generation += 1;
        let generation = state.library_vcs_generation;
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                let tools = ToolsOnPath::find();
                dirs.into_iter()
                    .map(|dir| {
                        let vcs = read_library_vcs_state(&dir, tools);
                        (dir, vcs)
                    })
                    .collect::<Vec<_>>()
            }),
            move |result| message(StartScreenMsg::LibraryVcsRead { generation, result }),
        )
    }

    /// Read the libraries' version control again when the Libraries page
    /// shows: Enable Version Control just finished on one of them.
    pub(crate) fn refresh_start_library_vcs(&mut self) -> Task<Message> {
        if self.start_screen_shown() && self.ui_state.start_screen.page == StartPage::Libraries {
            self.read_start_library_vcs()
        } else {
            Task::none()
        }
    }

    /// The read of `generation` finished. An older read's answer is dropped;
    /// a failed read keeps the last states and says why in Messages.
    pub(crate) fn finish_start_library_vcs_read(
        &mut self,
        generation: u64,
        result: Result<Vec<(PathBuf, LibraryVcsState)>, String>,
    ) {
        let state = &mut self.ui_state.start_screen;
        if generation != state.library_vcs_generation {
            return;
        }
        match result {
            Ok(states) => state.library_vcs = states.into_iter().collect(),
            Err(reason) => crate::diagnostics::log_warning(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::START_LIBRARIES_VCS_READ_FAILED,
                    "reason" => reason
                )
                .into_string(),
            ),
        }
    }

    /// "off" was pressed on the row of the library file `path`: open the
    /// library's Enable Version Control, as the project tree opens it. A
    /// press the last read no longer backs (the library is gone, or its
    /// folder has a `.git` now) does nothing.
    pub(crate) fn enable_start_library_vcs(&mut self, path: PathBuf) -> Task<Message> {
        let Some(dir) = library_folder(&path).map(Path::to_path_buf) else {
            return Task::none();
        };
        let off = self.ui_state.start_screen.library_vcs.get(&dir) == Some(&LibraryVcsState::Off);
        if !off || self.library.library_at(&path).is_none() {
            return Task::none();
        }
        self.open_library_vcs_dialog_at(path, dir)
    }

    /// The ⋯ button of the row of the library file `path` was pressed: its
    /// menu opens at the pointer.
    pub(crate) fn open_start_library_menu(&mut self, path: PathBuf) {
        self.close_all_context_menus();
        let (x, y) = self.interaction_state.last_mouse_pos;
        self.interaction_state.start_row_menu = Some(StartRowMenu {
            path,
            kind: StartRowKind::Library,
            x,
            y,
        });
    }

    /// Remove from Signex was picked: the confirm card asks first and says
    /// the files stay on disk. Only a mounted Global or Installed library.
    pub(crate) fn request_library_removal(&mut self, path: PathBuf) {
        self.interaction_state.start_row_menu = None;
        if !self.is_removable_library(&path) {
            return;
        }
        let name = self.library.library_label(&path);
        self.document_view.error_notice =
            Some(ErrorNotice::remove_library_from_signex(&name, &path));
    }

    /// The confirm was accepted: the library closes through the existing
    /// Close Library (at once, once its writes are done, or after its
    /// unsaved-edits prompt), and leaves the registry once it is unmounted
    /// (see [`Self::unregister_removed_library`]). A cancelled close removes
    /// nothing.
    pub(crate) fn confirm_library_removal(&mut self, path: PathBuf) -> Task<Message> {
        let asked = self
            .document_view
            .error_notice
            .as_ref()
            .and_then(|n| n.action.as_ref())
            .is_some_and(|a| matches!(a, NoticeAction::RemoveLibraryFromSignex { .. }));
        if asked {
            self.document_view.error_notice = None;
        }
        if !self.is_removable_library(&path) {
            return Task::none();
        }
        self.ui_state.start_screen.removing_library = Some(path.clone());
        self.handle_close_library(path)
    }

    /// Whether `path` is a mounted library Remove from Signex applies to.
    fn is_removable_library(&self, path: &Path) -> bool {
        self.library.library_at(path).is_some()
            && is_removable(self.library.mount_source_for(path, &[]))
    }

    /// The Close Library of a library being removed stopped (Cancel in its
    /// prompt, a failed Save All, a deferred close that found new edits):
    /// the removal is dropped too, so a later plain close does not remove it.
    pub(crate) fn drop_library_removal(&mut self, root: &Path) {
        let removing = &mut self.ui_state.start_screen.removing_library;
        if removing.as_deref() == Some(root) {
            *removing = None;
        }
    }

    /// Libraries were unmounted: the one being removed leaves the session's
    /// Installed list, and its Global entry leaves `global_libraries.toml`
    /// on a worker. Its files on disk are not touched.
    pub(super) fn unregister_removed_library(&mut self, unmounted: &[PathBuf]) -> Task<Message> {
        let Some(path) = self.ui_state.start_screen.removing_library.clone() else {
            return Task::none();
        };
        if !unmounted.contains(&path) || self.library.library_at(&path).is_some() {
            return Task::none();
        }
        self.ui_state.start_screen.removing_library = None;
        self.library.installed_libraries.retain(|p| *p != path);
        if !self.library.global_libraries.iter().any(|e| e.path == path) {
            return Task::none();
        }
        Task::perform(
            async move {
                crate::app::path_op_jobs::run_blocking(move || global_prefs::remove_path(&path))
                    .await
                    .and_then(|saved| saved)
            },
            |result| message(StartScreenMsg::GlobalLibraryUnregistered(result)),
        )
    }

    /// The Global entry's removal was written, or failed: a failure keeps the
    /// entry (the library mounts again at the next start) and says so.
    pub(crate) fn finish_global_library_unregister(
        &mut self,
        result: Result<Vec<GlobalLibraryEntry>, String>,
    ) {
        match result {
            Ok(saved) => self.library.global_libraries = saved,
            Err(e) => self.report_global_libraries_save_failure("remove library", &e),
        }
    }
}

#[cfg(test)]
#[path = "start_libraries_tests.rs"]
mod tests;
