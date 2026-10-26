//! Recovery of a sidecar an earlier run left behind (#1164): the check
//! when a schematic opens, the Restore / Discard prompt, and the startup
//! sweep. All disk work runs in `spawn_blocking`; the pure decisions are in
//! `crate::autosave::recovery`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use iced::Task;

use crate::app::state::ErrorNotice;
use crate::app::{Message, Signex};
use crate::autosave::{
    AutosaveMsg, InFlight, RecoveryChecked, RecoveryChoice, RecoveryOffer, RestoreLoaded, recovery,
};

impl Signex {
    /// A schematic tab just opened from disk: look, off the UI thread, for
    /// a recovery copy newer than its file. A copy that is older than the
    /// file or identical to it is removed by the same task, so a prompt is
    /// only ever offered for edits the file lacks.
    pub(crate) fn autosave_check_recovery(&mut self, path: &Path) -> Task<Message> {
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        let guard = self.document_state.autosave.track_job();
        check_task(dir, path.to_path_buf(), guard)
    }

    /// Startup: remove temp files a crashed write stranded in the
    /// recovery folder. Recovery copies themselves are reconciled when
    /// their document opens, never by age.
    pub(crate) fn autosave_startup_sweep(&mut self) -> Task<Message> {
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        let guard = self.document_state.autosave.track_job();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let _guard = guard;
                    recovery::sweep_stranded_temp_files(
                        &dir,
                        SystemTime::now(),
                        recovery::STRANDED_TEMP_MIN_AGE,
                    )
                })
                .await
                .map_err(|e| format!("the recovery sweep failed: {e}"))
                .and_then(|result| result)
            },
            |result| Message::Autosave(AutosaveMsg::StartupSwept(result)),
        )
    }

    pub(super) fn recovery_checked(&mut self, checked: RecoveryChecked) -> Task<Message> {
        let RecoveryChecked { path, result } = checked;
        match result {
            Ok(Some(copy)) => {
                let still_open = self.document_state.tabs.iter().any(|tab| tab.path == path);
                if still_open {
                    self.document_state.autosave.offer(RecoveryOffer {
                        path,
                        age_secs: copy.age_secs,
                    });
                }
            }
            Ok(None) => {}
            Err(reason) => self.report_autosave_failure(&reason),
        }
        Task::none()
    }

    pub(super) fn recovery_answered(&mut self, choice: RecoveryChoice) -> Task<Message> {
        let Some(offer) = self.document_state.autosave.pop_offer() else {
            return Task::none();
        };
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        match choice {
            // Esc: nothing is deleted or loaded; the copy is offered again
            // the next time the document opens. Until then no autosave may
            // overwrite it, and a save must not drop it.
            RecoveryChoice::Later => {
                self.document_state.autosave.defer(&offer.path);
                Task::none()
            }
            RecoveryChoice::Discard => self.start_remove(&dir, offer.path),
            RecoveryChoice::Restore => {
                if !self
                    .document_state
                    .tabs
                    .iter()
                    .any(|t| t.path == offer.path)
                {
                    self.report_restore_target_closed(&offer.path);
                    return Task::none();
                }
                let guard = self.document_state.autosave.track_job();
                restore_task(dir, offer.path, guard)
            }
        }
    }

    /// The copy was read: load it into the open document as unsaved edits.
    /// Nothing is written to the real file.
    pub(super) fn restore_loaded(&mut self, loaded: RestoreLoaded) -> Task<Message> {
        let RestoreLoaded { path, result } = loaded;
        let sheet = match result {
            Ok(sheet) => *sheet,
            Err(reason) => {
                self.document_state.error_notice = Some(ErrorNotice::recovery(format!(
                    "{reason}\nThe recovery copy was kept."
                )));
                return Task::none();
            }
        };
        if !self.document_state.engines.contains_key(&path) {
            // Closed while the copy was read; the copy stays for the next open.
            self.report_restore_target_closed(&path);
            return Task::none();
        }
        if self.document_state.dirty_paths.contains(&path) {
            self.document_state.error_notice = Some(ErrorNotice::recovery(format!(
                "{} was edited while the prompt was open, so its recovery copy was not loaded \
                 over those edits. The copy was kept.",
                file_label(&path)
            )));
            return Task::none();
        }
        if !self.activate_tab_for_path(&path) {
            return Task::none();
        }
        self.apply_loaded_schematic(Some(sheet), true, false, true);
        self.mark_active_document_dirty();
        // The copy is still on disk and now describes the open document:
        // track it so the next save removes it.
        self.document_state.autosave.adopt_on_disk(&path);
        Task::none()
    }
}

impl Signex {
    fn report_restore_target_closed(&mut self, path: &Path) {
        self.document_state.error_notice = Some(ErrorNotice::recovery(format!(
            "{} was closed, so its recovery copy was not restored. The copy was kept and is              offered again the next time the file is opened.",
            file_label(path)
        )));
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn check_task(dir: PathBuf, path: PathBuf, guard: InFlight) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                recovery::reconcile(&dir, &target, SystemTime::now())
            })
            .await
            .map_err(|e| format!("the recovery check failed: {e}"))
            .and_then(|result| result)
        },
        move |result| {
            Message::Autosave(AutosaveMsg::RecoveryChecked(RecoveryChecked {
                path: path.clone(),
                result,
            }))
        },
    )
}

fn restore_task(dir: PathBuf, path: PathBuf, guard: InFlight) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                recovery::load_sheet(&dir, &target).map(Box::new)
            })
            .await
            .map_err(|e| format!("the recovery read failed: {e}"))
            .and_then(|result| result)
        },
        move |result| {
            Message::Autosave(AutosaveMsg::RestoreLoaded(RestoreLoaded {
                path: path.clone(),
                result,
            }))
        },
    )
}
