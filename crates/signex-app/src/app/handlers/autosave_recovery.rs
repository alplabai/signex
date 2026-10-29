//! Recovery of a sidecar an earlier run left behind (#1164, #1185): the check
//! when a schematic, PCB, symbol or footprint opens, the Restore / Discard prompt, and the startup
//! sweep. All disk work runs in `spawn_blocking`; the pure decisions are in
//! `crate::autosave::recovery`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use iced::Task;

use crate::app::state::ErrorNotice;
use crate::app::{Message, Signex};
use crate::autosave::recovery::LoadError;
use crate::autosave::{
    AutosaveMsg, InFlight, RecoveryChecked, RecoveryChoice, RecoveryOffer, RestoreLoaded,
    RestoredDocument, recovery,
};

impl Signex {
    /// A document just opened from disk (a symbol or footprint editor):
    /// hold autosave back and start the recovery check. Held until the
    /// check reports, so no autosave write lands first and a rename waits
    /// for it (#1323).
    pub(crate) fn autosave_check_recovery(&mut self, path: &Path) -> Task<Message> {
        self.document_state.autosave.begin_check(path);
        self.start_recovery_check(path)
    }

    /// Look, off the UI thread, for a recovery copy of `path` that differs
    /// from its file; the caller already holds the check. A copy
    /// byte-identical to the file is removed by the same task; a differing
    /// copy is offered whatever its mtime, so a prompt is only ever shown
    /// for edits the file lacks.
    fn start_recovery_check(&mut self, path: &Path) -> Task<Message> {
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            // Nothing can be on disk without a recovery folder.
            self.document_state.autosave.finish_check(path);
            return Task::none();
        };
        let guard = self.document_state.autosave.track_job();
        // A move the live state owns into this path finishes on its own.
        let finish_owed = !self.document_state.autosave.move_owed(path);
        check_task(dir, path.to_path_buf(), finish_owed, guard)
    }

    /// Start the recovery check of every schematic tab opened since the last
    /// call. The opening path already held autosave back for each of them.
    pub(crate) fn start_due_recovery_checks(&mut self) -> Task<Message> {
        if self.document_state.recovery_check_due.is_empty() {
            return Task::none();
        }
        let due = std::mem::take(&mut self.document_state.recovery_check_due);
        Task::batch(due.iter().map(|path| self.start_recovery_check(path)))
    }

    /// Startup, before any document opens: finish the recovery-copy moves a
    /// previous run left journalled (#1510). A move that still fails stays
    /// journalled and its copy stays untouched at the old key, for the next
    /// start to retry. It is not handed to the live state: the recovery check
    /// of a document opened at the new path retries it first
    /// ([`recovery::reconcile`]), and a move landing later would put the copy
    /// under a key the session may already write to.
    pub(crate) fn autosave_finish_pending_moves(&mut self) {
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return;
        };
        crate::autosave::pending_move::finish_all(&dir);
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
        // An offer queued below takes over the protection of the copy.
        self.document_state.autosave.finish_check(&path);
        match result {
            Ok(Some(copy)) => {
                // A parked engine (tab closed while dirty) is still open here.
                let still_open = self.is_open_document(&path);
                if still_open {
                    self.document_state.autosave.offer(RecoveryOffer {
                        path,
                        age_secs: copy.age_secs,
                        older_than_file: copy.older_than_file,
                    });
                }
            }
            Ok(None) => {}
            Err(reason) => {
                // The copy could not be judged, so it may hold edits nobody
                // has seen: no autosave write may replace it. Closing the
                // document drops the pause; reopening it checks again.
                self.document_state.autosave.defer(&path);
                self.report_autosave_failure(&format!(
                    "{reason}. The recovery copy was not touched and autosave is paused for \
                     this document; close and reopen it to check the copy again."
                ));
            }
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
                    // A dirty tab closed meanwhile leaves its engine parked
                    // and still autosaved: keep the copy protected (the
                    // offer blocks writes) and offered for the reopen.
                    if self.is_open_document(&offer.path) {
                        let name = file_label(&offer.path);
                        self.document_state.autosave.offer(offer);
                        self.report_recovery_failure(format!(
                            "Restore needs the document open: reopen {name} to restore. The \
                             copy is kept and autosave stays paused for it."
                        ));
                    } else {
                        self.report_restore_target_closed(&offer.path);
                    }
                    return Task::none();
                }
                // After a rename the copy may still sit under the old name.
                if self.document_state.autosave.move_owed(&offer.path) {
                    let name = file_label(&offer.path);
                    self.document_state.autosave.offer(offer);
                    self.report_recovery_failure(format!(
                        "The recovery copy of {name} is still being moved to the file's new \
                         name. It was kept; choose Restore again in a moment."
                    ));
                    // Start the move now rather than on the next tick, which
                    // never comes while autosave is off.
                    return self.autosave_flush_moves();
                }
                self.document_state.autosave.begin_restore(&offer.path);
                // The copy is read off-thread; no write may replace it
                // meanwhile, and none may if the restore is then refused.
                self.document_state.autosave.defer(&offer.path);
                let guard = self.document_state.autosave.track_job();
                restore_task(dir, offer, guard)
            }
        }
    }

    /// The copy was read: load it into the open document as unsaved edits.
    /// Nothing is written to the real file.
    pub(super) fn restore_loaded(&mut self, loaded: RestoreLoaded) -> Task<Message> {
        let RestoreLoaded {
            path,
            mut offer,
            result,
        } = loaded;
        let current = self.document_state.autosave.finish_restore(&path);
        if current != path {
            // Renamed while the copy was read: the copy moves to the new
            // path, so a read that missed it says nothing about it.
            offer.path = current.clone();
            if result.is_err() {
                let name = file_label(&current);
                self.document_state.autosave.offer(offer);
                self.report_recovery_failure(format!(
                    "{name} was renamed while its recovery copy was being read. The copy was \
                     kept and is offered again."
                ));
                return Task::none();
            }
        }
        let path = current;
        let restored = match result {
            Ok(restored) => restored,
            Err(LoadError::Invalid(reason)) => {
                // A copy that cannot be parsed protects nothing, and keeping
                // autosave off for it would leave this session's edits
                // unprotected: autosave resumes and replaces it. The reason
                // already says so.
                self.document_state.autosave.allow_writes(&path);
                self.report_recovery_failure(reason);
                return Task::none();
            }
            Err(LoadError::Missing(reason)) => {
                // Nothing left to protect or offer: autosave resumes.
                self.document_state.autosave.allow_writes(&path);
                self.report_recovery_failure(format!(
                    "{reason}\nThe recovery copy is gone, so there is nothing to restore. \
                     Autosave resumes for this document."
                ));
                return Task::none();
            }
            Err(LoadError::Unreadable(reason)) => {
                // The copy may be intact (a lock, permissions): do not let
                // autosave replace it. Offer it again and stay paused.
                self.document_state.autosave.offer(offer);
                self.report_recovery_failure(format!(
                    "{reason}\nThe copy was kept and is offered again. Autosave stays paused \
                     for this document until you answer."
                ));
                return Task::none();
            }
        };
        let is_open = match &restored {
            RestoredDocument::Schematic(_) => self.document_state.engines.contains_key(&path),
            RestoredDocument::Symbol(_) => self.document_state.symbol_editors.contains_key(&path),
            RestoredDocument::Footprint(_) => {
                self.document_state.footprint_editors.contains_key(&path)
            }
            RestoredDocument::Pcb(_) => self.is_open_pcb(&path),
        };
        if !is_open {
            // Closed while the copy was read; the copy stays for the next open.
            self.document_state.autosave.allow_writes(&path);
            self.report_restore_target_closed(&path);
            return Task::none();
        }
        if self.document_state.dirty_paths.contains(&path)
            || self.is_dirty_primitive_editor(&path)
            || self.is_dirty_pcb(&path)
        {
            // Both versions are worth keeping: the edits live in the open
            // document, the earlier ones in the copy. Autosave stays off for
            // this document (the one sidecar is that copy) and the prompt
            // comes back, so Discard (which resumes autosave) is one click away.
            self.document_state.autosave.offer(offer);
            self.report_recovery_failure(format!(
                "{} was edited while the prompt was open, so its recovery copy was not loaded \
                 over those edits. The copy was kept and is offered again. Autosave stays \
                 paused for this document until you answer, so neither version is overwritten.",
                file_label(&path)
            ));
            return Task::none();
        }
        if !self.activate_tab_for_path(&path) {
            self.document_state.autosave.offer(offer);
            self.report_recovery_failure(format!(
                "The tab of {} could not be activated, so its recovery copy was not restored. \
                 The copy was kept and is offered again. Autosave stays paused for this \
                 document until you answer.",
                file_label(&path)
            ));
            return Task::none();
        }
        match restored {
            RestoredDocument::Schematic(sheet) => {
                self.apply_loaded_schematic(Some(*sheet), true, false, true);
                self.mark_active_document_dirty();
            }
            RestoredDocument::Symbol(file) => {
                self.restore_symbol_copy(&path, *file);
                self.sync_editor_dirty_paths();
            }
            RestoredDocument::Footprint(file) => {
                self.restore_footprint_copy(&path, *file);
                self.sync_editor_dirty_paths();
            }
            RestoredDocument::Pcb(board) => {
                self.restore_pcb_copy(&path, *board);
            }
        }
        // The copy is still on disk and now describes the open document:
        // track it so the next save removes it.
        self.document_state.autosave.adopt_on_disk(&path);
        // The document now holds the copy, so autosave may write again.
        self.document_state.autosave.allow_writes(&path);
        Task::none()
    }
}

impl Signex {
    /// A schematic engine (possibly parked), a symbol editor or a footprint
    /// editor holds `path`.
    fn is_open_document(&self, path: &Path) -> bool {
        let docs = &self.document_state;
        docs.engines.contains_key(path)
            || self.is_open_pcb(path)
            || docs.symbol_editors.contains_key(path)
            || docs.footprint_editors.contains_key(path)
    }

    fn report_restore_target_closed(&mut self, path: &Path) {
        self.report_recovery_failure(format!(
            "{} was closed, so its recovery copy was not restored. The copy was kept and is \
             offered again the next time the file is opened.",
            file_label(path)
        ));
    }

    /// A Restore that did not happen: the dialog tells the user, and the
    /// Messages panel keeps the record after the dialog is dismissed.
    fn report_recovery_failure(&mut self, text: String) {
        crate::diagnostics::log_warning(format!("Recovery: {}", text.replace('\n', " ")));
        self.document_state.error_notice = Some(ErrorNotice::recovery(text));
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn check_task(dir: PathBuf, path: PathBuf, finish_owed: bool, guard: InFlight) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                recovery::reconcile_with(&dir, &target, SystemTime::now(), finish_owed)
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

fn restore_task(dir: PathBuf, offer: RecoveryOffer, guard: InFlight) -> Task<Message> {
    let path = offer.path.clone();
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                recovery::load_document(&dir, &target)
            })
            .await
            .map_err(|e| LoadError::Unreadable(format!("the recovery read failed: {e}")))
            .and_then(|result| result)
        },
        move |result| {
            Message::Autosave(AutosaveMsg::RestoreLoaded(RestoreLoaded {
                path: path.clone(),
                offer: offer.clone(),
                result,
            }))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1208 — a Restore that fails reaches the Messages panel as well as
    /// the dialog.
    #[test]
    fn a_failed_restore_is_logged_to_messages_and_shown_in_the_dialog() {
        let needle = "restore-failure-needle-1208";
        let before = crate::diagnostics::logged_count(needle);
        let (mut app, _boot) = Signex::new();

        let _task = app.restore_loaded(RestoreLoaded {
            path: PathBuf::from("a.snxsch"),
            offer: RecoveryOffer {
                path: PathBuf::from("a.snxsch"),
                age_secs: 0,
                older_than_file: false,
            },
            result: Err(LoadError::Invalid(format!(
                "{needle}: could not read recovery copy a.recovery"
            ))),
        });

        assert!(app.document_state.error_notice.is_some());
        assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
    }

    /// A refused Restore into a tab that cannot be activated says so too.
    #[test]
    fn a_restore_into_a_missing_tab_is_logged_to_messages() {
        let before = crate::diagnostics::logged_count("closed, so its recovery copy");
        let (mut app, _boot) = Signex::new();
        app.report_restore_target_closed(Path::new("gone-1208.snxsch"));
        assert_eq!(
            crate::diagnostics::logged_count("closed, so its recovery copy"),
            before + 1
        );
    }
}
