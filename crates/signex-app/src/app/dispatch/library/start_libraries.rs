//! The Start screen's Libraries page actions: the off-thread read of each
//! library's version control and Enable Version Control from its row
//! (#2591). Design:
//! `docs/architecture/start-screen-libraries.md`.

use std::path::{Path, PathBuf};

use super::*;
use crate::start_screen::{StartPage, StartScreenMsg, library_folder, message};
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
}

#[cfg(test)]
#[path = "start_libraries_tests.rs"]
mod tests;
