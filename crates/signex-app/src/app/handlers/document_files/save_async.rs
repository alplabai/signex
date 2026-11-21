//! Off-thread schematic save (#859).
//!
//! `update` serialises the document (a cheap, consistent snapshot) and
//! returns a `Task` that does the blocking write. Until the completion
//! message arrives nothing is marked clean: the dirty flag clears only when
//! the engine is still at the revision the snapshot was taken at, so an
//! edit made while the write ran keeps the document dirty. A failed write
//! never touches the flag and reaches the Messages panel.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use iced::Task;

use crate::app::{FileMsg, Message, Signex, TabKind};
use crate::persist::{DocumentSaveDone, SaveOutcome};

/// How a schematic write is compared with its file before it lands.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DiskCheck {
    /// Refuse when the file changed or is gone (Ctrl+S). A Save As to a new
    /// target has no baseline, so nothing is compared.
    Compare,
    /// Refuse when the file changed or its folder is gone; write it when only
    /// the file is gone.
    CompareWritingMissing,
    /// Write without comparing.
    Skip,
}

impl Signex {
    /// Snapshot the engine behind `save_as_from` (or `target` for a plain
    /// save) and return the task that writes it to `target`.
    pub(crate) fn begin_schematic_save(
        &mut self,
        target: PathBuf,
        save_as_from: Option<PathBuf>,
    ) -> Result<Task<Message>> {
        self.begin_schematic_save_checked(target, save_as_from, DiskCheck::Compare)
    }

    /// A plain save the user chose to run over a changed file: this one write
    /// is not compared with disk, and a failure leaves the baseline as it was.
    pub(crate) fn begin_schematic_save_unchecked(
        &mut self,
        target: PathBuf,
    ) -> Result<Task<Message>> {
        self.begin_schematic_save_checked(target, None, DiskCheck::Skip)
    }

    /// File ▸ Save As onto the sheet's own file: a plain save compared with
    /// disk like Ctrl+S, except that a file deleted or moved away is written,
    /// as Save As always did.
    pub(crate) fn begin_schematic_save_onto_own_file(
        &mut self,
        target: PathBuf,
    ) -> Result<Task<Message>> {
        self.begin_schematic_save_checked(target, None, DiskCheck::CompareWritingMissing)
    }

    fn begin_schematic_save_checked(
        &mut self,
        target: PathBuf,
        save_as_from: Option<PathBuf>,
        check: DiskCheck,
    ) -> Result<Task<Message>> {
        let source = save_as_from.clone().unwrap_or_else(|| target.clone());
        // A rename or delete still moving either file would recreate it or
        // lose these bytes (#859).
        self.refuse_save_while_path_op(&source)?;
        self.refuse_save_while_path_op(&target)?;
        self.refuse_while_project_moves(&source, "save")?;
        self.refuse_while_project_moves(&target, "save")?;
        let engine = self
            .document_state
            .engines
            .get(&source)
            .with_context(|| format!("no open schematic at {}", source.display()))?;
        let revision = engine.revision();
        let measure = crate::schematic_runtime::text_width::live_extent_measure();
        // A Save As makes a new document: the copy is written with an id of
        // its own, and the document takes it when the write lands (#2172). A
        // Save As onto another spelling of its own file (a case variant) is
        // the same document: it keeps its id.
        let new_document = save_as_from.as_ref().is_some_and(|from| {
            let fold = self.identity_fold_for(from);
            crate::app::state::scope::path_key_with(from, fold)
                != crate::app::state::scope::path_key_with(&target, fold)
        });
        let encoded = if new_document {
            engine.serialize_as_copy_with_extents(
                signex_types::document_id::DocumentId::new().as_uuid(),
                measure,
            )
        } else {
            engine.serialize_with_extents(measure)
        };
        let bytes = encoded
            .with_context(|| format!("encode schematic {}", source.display()))?
            .into_bytes();
        // A plain save is compared with what the file held when the sheet was
        // loaded or last saved; a Save As target was confirmed by the dialog.
        let expected = (check != DiskCheck::Skip && save_as_from.is_none())
            .then(|| self.document_state.document_saves.baseline(&target))
            .flatten();
        let mut job = self
            .document_state
            .document_saves
            .begin(target.clone(), bytes)
            .expecting(expected);
        if check == DiskCheck::CompareWritingMissing {
            job = job.writing_a_missing_file();
        }
        let hold = self.document_state.autosave.hold_for_save();
        Ok(Self::spawn_document_write(
            job,
            target,
            revision,
            save_as_from,
            hold,
        ))
    }

    /// Snapshot the board of the PCB at `path` (its tab's, or the one parked
    /// when its dirty tab closed) and return the task that writes it
    /// atomically to its `.snxpcb`. `Ok(None)` means there is nothing to
    /// write: a clean tab must not rewrite the open-time snapshot over a file
    /// another tool or a git operation changed, and a board opened read-only
    /// (newer minor) is refused with an error. The completion routes through the
    /// same `DocumentSaved` message as a schematic save.
    pub(crate) fn begin_pcb_save(&mut self, path: PathBuf) -> Result<Option<Task<Message>>> {
        self.begin_pcb_save_checked(path, true)
    }

    /// The PCB twin of [`Self::begin_schematic_save_unchecked`].
    pub(crate) fn begin_pcb_save_unchecked(
        &mut self,
        path: PathBuf,
    ) -> Result<Option<Task<Message>>> {
        self.begin_pcb_save_checked(path, false)
    }

    fn begin_pcb_save_checked(
        &mut self,
        path: PathBuf,
        compare_with_disk: bool,
    ) -> Result<Option<Task<Message>>> {
        self.refuse_save_while_path_op(&path)?;
        self.refuse_while_project_moves(&path, "save")?;
        if !self.document_state.holds_pcb(&path) {
            anyhow::bail!("no open PCB at {}", path.display());
        }
        if !self.document_state.is_document_dirty(&path) {
            return Ok(None);
        }
        let revision = self
            .document_state
            .pcb_revisions
            .get(&path)
            .copied()
            .unwrap_or(0);
        let board = self.document_state.pcb_board(&path).with_context(|| {
            format!(
                "{} could not be re-read after a History restore, so the board in memory \
                     is stale; close and reopen the tab (saving now would undo the restore)",
                path.display()
            )
        })?;
        if board.unknown.newer_minor_source.is_some() {
            anyhow::bail!(
                "{} opened read-only (newer format) — not saved",
                path.display()
            );
        }
        let bytes = signex_types::format::SnxPcb::new(board.clone())
            .write_string()
            .with_context(|| format!("encode PCB {}", path.display()))?
            .into_bytes();
        let expected = compare_with_disk
            .then(|| self.document_state.document_saves.baseline(&path))
            .flatten();
        let job = self
            .document_state
            .document_saves
            .begin(path.clone(), bytes)
            .expecting(expected);
        let hold = self.document_state.autosave.hold_for_save();
        Ok(Some(Self::spawn_document_write(
            job, path, revision, None, hold,
        )))
    }

    fn spawn_document_write(
        job: crate::persist::DocumentSaveJob,
        target: PathBuf,
        revision: u64,
        save_as_from: Option<PathBuf>,
        hold: crate::autosave::SaveHold,
    ) -> Task<Message> {
        let bytes = job.bytes();
        let sequence = job.sequence();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .map_err(|join_error| format!("the save task failed: {join_error}"))
                    .and_then(|result| result)
            },
            move |result| {
                Message::File(FileMsg::DocumentSaved(DocumentSaveDone {
                    path: target.clone(),
                    revision,
                    save_as_from: save_as_from.clone(),
                    bytes: Arc::clone(&bytes),
                    sequence,
                    result,
                    recovery_hold: Some(hold.clone()),
                }))
            },
        )
    }

    /// A schematic write finished. Success clears the dirty marker only if
    /// the document is unchanged since the snapshot; the project git commit
    /// is queued here, after the file is on disk.
    pub(crate) fn handle_document_saved(&mut self, done: DocumentSaveDone) -> Task<Message> {
        self.document_state.document_saves.finish(&done.path);
        let outcome_path = done.path.clone();
        let DocumentSaveDone {
            path,
            revision,
            save_as_from,
            bytes,
            sequence,
            result,
            // Dropped when this function returns, i.e. after the copy
            // removal below was registered with the quit barrier.
            recovery_hold: _recovery_hold,
        } = done;
        let saved = crate::app::state::CommitBytes { bytes, sequence };
        let reason = match result {
            Ok(SaveOutcome::Written) => None,
            // A newer snapshot of this path reached disk first; its own
            // completion decides dirty/git. Nothing of ours landed.
            Ok(SaveOutcome::Superseded) if save_as_from.is_none() => {
                self.document_state.document_saves.defer_superseded(path);
                self.warn_unwritten_spellings(&outcome_path);
                return Task::none();
            }
            Ok(SaveOutcome::Superseded) => Some(format!(
                "{} was not written: a newer write to it landed first",
                path.display()
            )),
            Ok(SaveOutcome::DiskChanged(drift)) => {
                self.handle_save_found_disk_drift(path, drift);
                return Task::none();
            }
            Err(reason) => Some(reason),
        };
        if let Some(reason) = reason {
            let context = if save_as_from.is_some() {
                "Failed to save active document as"
            } else {
                "Failed to save active document"
            };
            self.document_state
                .document_saves
                .record_failure(path, reason.clone());
            self.report_save_failure(context, &anyhow::anyhow!(reason));
            // The file is unchanged, so an Undo back to the save point that
            // the write was holding dirty may now read clean.
            self.document_state.sync_history_dirty(&outcome_path);
            return Task::none();
        }
        self.document_state.document_saves.clear_failure(&path);
        self.note_written_baseline(&path, &saved);
        // The real file is now at least as new as any recovery copy of
        // the document it was saved from (or over).
        let recovery_keys = [Some(path.clone()), save_as_from.clone()];
        let mut project_write = Task::none();
        match save_as_from {
            None if is_pcb_path(&path) => self.mark_pcb_saved(&path, revision, saved),
            None => self.mark_saved(&path, revision, saved),
            Some(from) => {
                self.document_state
                    .document_saves
                    .forget_disk_records(&from);
                self.record_real_path_of(&path);
                let owner = self.waiver_owner_of(&from);
                project_write = self.apply_saved_as(&from, path.clone(), revision);
                // Only when the tab was re-keyed onto the copy; a Save As
                // that bailed leaves the document, and the id, as they were.
                let rekeyed = self.document_state.engines.contains_key(&path)
                    && !self.document_state.engines.contains_key(&from);
                if rekeyed {
                    // The document left `from`; a failure recorded for it
                    // (a save refused over a changed file) is not its own now.
                    self.document_state.document_saves.clear_failure(&from);
                    self.adopt_saved_copy_id(&path, &saved, owner);
                }
            }
        }
        self.warn_unwritten_spellings(&outcome_path);
        self.refresh_panel_ctx();
        let mut tasks: Vec<Task<Message>> = recovery_keys
            .iter()
            .flatten()
            .map(|key| self.autosave_after_save(key))
            .collect();
        tasks.push(project_write);
        tasks.push(self.refresh_project_thumbnail(&path));
        Task::batch(tasks)
    }

    /// The Start screen preview of the project that owns the document just
    /// saved at `saved` (mockup row 1): the project's board when it has one and
    /// that board was saved, else its root sheet when that is loaded. The
    /// render is kept at once and written to the cache in a task; any other
    /// save leaves the preview as it is.
    pub(crate) fn refresh_project_thumbnail(&mut self, saved: &std::path::Path) -> Task<Message> {
        let Some(project) = self.owning_project(saved) else {
            return Task::none();
        };
        let fold = project.case_fold;
        let same = |a: &std::path::Path, b: &std::path::Path| {
            crate::app::state::scope::path_key_with(a, fold)
                == crate::app::state::scope::path_key_with(b, fold)
        };
        let thumbnail = match Self::project_board(project) {
            // The first board in project order: a project names one board.
            Some(board_path) if same(saved, board_path.as_path()) => self
                .document_state
                .pcb_board(saved)
                .map(crate::project_thumbnail::snapshot::board),
            Some(_) => None,
            None => crate::app::project_sheets::project_root_sheet_path(project).and_then(|root| {
                self.document_state
                    .engines
                    .iter()
                    .find(|(path, _)| same(path.as_path(), root.as_path()))
                    .map(|(_, engine)| crate::project_thumbnail::snapshot::sheet(engine.document()))
            }),
        };
        let project_path = project.path.clone();
        match thumbnail {
            Some(thumbnail) => self.keep_project_thumbnail(project_path, thumbnail),
            None => Task::none(),
        }
    }

    /// A Save As landed and the engine now stands for the new file: it takes
    /// the document id the written copy carries, so the document in memory is
    /// the one on disk (#2172).
    fn adopt_saved_copy_id(
        &mut self,
        path: &std::path::Path,
        saved: &crate::app::state::CommitBytes,
        owner: Option<crate::app::state::ProjectId>,
    ) {
        let Some(id) = signex_types::format::document_id_in_header("snxsch", &saved.bytes) else {
            return;
        };
        let Some(engine) = self.document_state.engines.get_mut(path) else {
            return;
        };
        let old = engine.document().uuid;
        engine.reassign_document_id(id.as_uuid());
        // The project follows the copy; its net waivers name the sheet by id.
        if !old.is_nil() && old != id.as_uuid() {
            self.follow_waivers(
                owner,
                old,
                id.as_uuid(),
                crate::app::document_ids::Follow::Move,
            );
        }
    }

    /// Once no write to the file `path` names (under any spelling) is still owed, warn about
    /// every spelling whose snapshot lost the ordering race and is still
    /// modified: a sibling spelling's newer bytes are on disk instead of its
    /// own, so it must be saved again. A same-document rapid Ctrl+S loser is
    /// not listed: its winner's completion has cleaned it by now. Runs from
    /// both completions, so the warning fires in either completion order.
    pub(crate) fn warn_unwritten_spellings(&mut self, path: &std::path::Path) {
        for path in self.document_state.document_saves.take_superseded(path) {
            if self.document_state.is_document_dirty(&path) {
                crate::diagnostics::log_warning(format!(
                    "[save] {} was not written: a newer write to the same file landed first; \
                     it stays modified",
                    path.display()
                ));
            }
        }
    }

    /// Wait for every document write already in flight, then run `next`.
    /// Quit goes through here so the process never exits mid-write.
    pub(crate) fn wait_for_document_saves_then(&mut self, next: Task<Message>) -> Task<Message> {
        let barrier = self.document_state.document_saves.barrier();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || barrier.wait())
                    .await
                    .map_err(|join_error| {
                        format!("waiting for the document write failed: {join_error}")
                    })
            },
            |result| match result {
                Ok(()) => Message::noop(),
                Err(reason) => Message::File(FileMsg::DocumentSaveWaitFailed(reason)),
            },
        )
        .chain(next)
    }

    pub(crate) fn mark_saved(
        &mut self,
        path: &std::path::Path,
        revision: u64,
        saved: crate::app::state::CommitBytes,
    ) {
        crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
        // The file on disk now differs from what any earlier read saw.
        self.document_state.note_documents_changed();
        // No engine backs `path` any more (renamed or closed meanwhile): the
        // bytes landed, but there is nothing to mark clean or to commit
        // under a name that no longer describes the document.
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        // A different revision means the engine was replaced (History
        // restore) or edited; "Save X" would mislabel what is committed.
        if engine.revision() != revision {
            // The bytes that landed are a snapshot the document has moved
            // past: no undo or redo step reproduces the file now.
            // An undo during the write may have cleared the mark against the
            // old save point: the file holds the snapshot, the document
            // does not, so it is dirty again.
            if let Some(engine) = self.document_state.engines.get_mut(path) {
                engine.forget_save_point();
            }
            self.document_state.sync_history_dirty(path);
            return;
        }
        self.finish_clean_save(path, saved);
    }

    /// A PCB write landed. Like `mark_saved`, a changed board revision means
    /// the bytes are stale: stay dirty and queue no "Save" commit. Routed by
    /// path, so a tab closed mid-write (no tab left) still commits.
    fn mark_pcb_saved(
        &mut self,
        path: &std::path::Path,
        revision: u64,
        saved: crate::app::state::CommitBytes,
    ) {
        crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
        // A missing key reads as 0 on both sides (begin uses the same
        // fallback), so it can never pin a tab dirty.
        let current = self
            .document_state
            .pcb_revisions
            .get(path)
            .copied()
            .unwrap_or(0);
        if current != revision {
            // The file now holds a snapshot the board has moved past: no
            // history step reproduces it, so undo must not read clean.
            self.document_state.forget_board_save_point(path);
            self.document_state.sync_history_dirty(path);
            return;
        }
        self.finish_clean_save(path, saved);
    }

    /// The bytes for `path` landed and match the document: clear its dirty
    /// markers, release a parked engine nothing holds open, and queue the
    /// project git commit.
    fn finish_clean_save(&mut self, path: &std::path::Path, saved: crate::app::state::CommitBytes) {
        self.document_state.mark_document_clean(path);
        // The tab closed mid-save and parked the engine because it was
        // dirty; now it is clean nothing needs it.
        if !self.document_is_held_open(path) {
            self.document_state.remove_engine(path);
            self.document_state.document_saves.forget_disk_records(path);
        }
        let label = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        self.commit_bytes_to_project_git(path, &format!("Save {label}"), saved);
    }

    pub(crate) fn document_is_held_open(&self, path: &std::path::Path) -> bool {
        self.document_state.tabs.iter().any(|tab| tab.path == path)
            || self.undocked_sheet_paths().contains(path)
    }

    /// The documents an undocked window shows: with the tabs, what holds a
    /// document open.
    pub(crate) fn undocked_sheet_paths(&self) -> std::collections::HashSet<PathBuf> {
        (self.ui_state.windows.values())
            .filter_map(|kind| match kind {
                crate::app::state::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    /// Save As landed: re-key and retitle the tab, but only if it is still
    /// the schematic the dialog was opened for.
    fn apply_saved_as(
        &mut self,
        from: &std::path::Path,
        to: PathBuf,
        revision: u64,
    ) -> Task<Message> {
        // The window that started the Save As shows the document, whichever
        // tab the main window has active now (#1794).
        if let Some(window) = self.undocked_schematic_window(from) {
            return self.run_in_window_document(window, |app| {
                app.apply_saved_as_to_active(from, to.clone(), revision)
            });
        }
        self.apply_saved_as_to_active(from, to, revision)
    }

    fn apply_saved_as_to_active(
        &mut self,
        from: &std::path::Path,
        to: PathBuf,
        revision: u64,
    ) -> Task<Message> {
        let still_active = matches!(
            self.document_state.tabs.get(self.document_state.active_tab),
            Some(tab) if matches!(tab.kind, TabKind::Schematic) && tab.path == from
        );
        if !still_active || self.save_as_target_is_other_open_document(&to) {
            self.report_save_failure(
                "Failed to save active document as",
                &anyhow::anyhow!(
                    "the file was written, but the tab was switched or the target is being \
                     opened or was opened meanwhile, so the tab stays at {} (written: {})",
                    super::save::file_label(from),
                    super::save::file_label(&to)
                ),
            );
            // The panel keeps ~160 bytes of a record, so the consequence gets
            // its own short line. An open that read `to` before the write
            // shows the old bytes; saving that tab would overwrite this write.
            // Only when such a tab exists: a plain tab switch leaves none.
            if self.save_as_target_is_other_open_document(&to) {
                crate::diagnostics::log_warning(format!(
                    "[save-as] A tab for {} may show its previous content, not what was \
                     just written; do not save it over the Save As result",
                    super::save::file_label(&to)
                ));
            }
            return Task::none();
        }
        // The project that listed the sheet, found while the engine is still
        // keyed by the old path.
        let owner = self.document_state.document_project(from).map(|p| p.id);
        // Saved only if the document has not changed since the snapshot
        // taken at `revision`.
        let renamed = self.rekey_active_schematic(to.clone(), |engine| {
            if engine.revision() == revision {
                engine.mark_saved();
                false
            } else {
                // The new file holds a snapshot the document has moved past (an
                // undo back to an older state counts), so no undo or redo step
                // reproduces it and the document is not saved.
                engine.forget_save_point();
                true
            }
        });
        if renamed {
            crate::diagnostics::log_info(format!("[save-as] Wrote {}", to.display()));
            return self.follow_save_as_in_project(owner, from, &to);
        }
        Task::none()
    }
}

/// Routed by extension, not by an open tab, so a PCB whose tab closed
/// mid-write still logs and commits. Any letter case, as open accepts it, so
/// a board opened as `Board.SNXPCB` completes as a board (#1832).
pub(crate) fn is_pcb_path(path: &std::path::Path) -> bool {
    use crate::app::file_extension::has_extension;
    has_extension(path, "snxpcb") || has_extension(path, "standard_pcb")
}

/// The project preview a save rebuilds (mockup row 1).
#[cfg(test)]
mod thumbnail_tests {
    use std::path::PathBuf;

    use signex_types::schematic::SchematicSheet;

    use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
    use crate::project_thumbnail::tests::small_sheet;
    use crate::project_thumbnail::{Ink, ThumbnailKind};

    fn project_path() -> PathBuf {
        PathBuf::from("/w/thumb/ProjectA.snxprj")
    }

    #[test]
    fn saving_a_sheet_of_a_project_without_a_board_renders_its_root_sheet() {
        let mut app = app_workspace("/w/thumb", &["Top.snxsch", "Sub.snxsch"]);
        let root = PathBuf::from("/w/thumb/Top.snxsch");
        let sub = PathBuf::from("/w/thumb/Sub.snxsch");
        open_with(&mut app.document_state, &root, small_sheet());
        open_with(&mut app.document_state, &sub, SchematicSheet::default());
        let _task = app.refresh_project_thumbnail(&sub);
        let kept = app
            .ui_state
            .project_thumbnails
            .get(&project_path())
            .expect("rendered");
        assert_eq!(kept.kind, ThumbnailKind::Sheet);
        let wires = kept.primitives.iter().filter(|p| p.ink == Ink::Wire);
        assert_eq!(
            wires.count(),
            1,
            "the root sheet's wire, not the empty sub-sheet"
        );
        assert!(kept.primitives.iter().any(|p| p.ink == Ink::PowerPort));
    }

    #[test]
    fn a_sheet_save_leaves_the_preview_of_a_project_with_a_board() {
        let mut app = app_workspace("/w/thumb", &["Top.snxsch"]);
        app.document_state.projects[0].data.pcb_file = Some("Board.snxpcb".to_string());
        let root = PathBuf::from("/w/thumb/Top.snxsch");
        open_with(&mut app.document_state, &root, small_sheet());
        let _task = app.refresh_project_thumbnail(&root);
        assert!(app.ui_state.project_thumbnails.is_empty());
    }

    #[test]
    fn a_root_sheet_that_is_not_loaded_renders_nothing() {
        let mut app = app_workspace("/w/thumb", &["Top.snxsch", "Sub.snxsch"]);
        let sub = PathBuf::from("/w/thumb/Sub.snxsch");
        open_with(&mut app.document_state, &sub, small_sheet());
        let _task = app.refresh_project_thumbnail(&sub);
        assert!(app.ui_state.project_thumbnails.is_empty());
    }

    #[test]
    fn a_loose_sheet_renders_nothing() {
        let mut app = app_workspace("/w/thumb", &["Top.snxsch"]);
        let loose = PathBuf::from("/w/elsewhere/Loose.snxsch");
        open_with(&mut app.document_state, &loose, small_sheet());
        let _task = app.refresh_project_thumbnail(&loose);
        assert!(app.ui_state.project_thumbnails.is_empty());
    }
}
