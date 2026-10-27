//! Off-thread schematic save (#859).
//!
//! `update` serialises the document (a cheap, consistent snapshot) and
//! returns a `Task` that does the blocking write. Until the completion
//! message arrives nothing is marked clean: the dirty flag clears only when
//! the engine is still at the revision the snapshot was taken at, so an
//! edit made while the write ran keeps the document dirty. A failed write
//! never touches the flag and reaches the Messages panel.

use std::path::PathBuf;

use anyhow::{Context, Result};
use iced::Task;

use crate::app::{FileMsg, Message, Signex, TabKind};
use crate::persist::{DocumentSaveDone, SaveOutcome};

impl Signex {
    /// Snapshot the engine behind `save_as_from` (or `target` for a plain
    /// save) and return the task that writes it to `target`.
    pub(crate) fn begin_schematic_save(
        &mut self,
        target: PathBuf,
        save_as_from: Option<PathBuf>,
    ) -> Result<Task<Message>> {
        let source = save_as_from.clone().unwrap_or_else(|| target.clone());
        self.refuse_while_project_moves(&source, "save")?;
        self.refuse_while_project_moves(&target, "save")?;
        let engine = self
            .document_state
            .engines
            .get(&source)
            .with_context(|| format!("no open schematic at {}", source.display()))?;
        let revision = engine.revision();
        let net_color_revision = self.net_color_revision_of(&source);
        let bytes = engine
            .serialize_with_net_colors(&self.ui_state.wire_color_overrides)
            .with_context(|| format!("encode schematic {}", source.display()))?
            .into_bytes();
        let job = self
            .document_state
            .document_saves
            .begin(target.clone(), bytes);
        Ok(Self::spawn_document_write(
            job,
            target,
            revision,
            net_color_revision,
            save_as_from,
        ))
    }

    /// Snapshot the active PCB tab's board and return the task that writes
    /// it atomically to its `.snxpcb`. `Ok(None)` means there is nothing to
    /// write: a clean tab must not rewrite the open-time snapshot over a file
    /// another tool or a git operation changed, and a board opened read-only
    /// (newer minor) is refused with an error. The completion routes through the
    /// same `DocumentSaved` message as a schematic save.
    pub(crate) fn begin_pcb_save(&mut self, path: PathBuf) -> Result<Option<Task<Message>>> {
        self.refuse_while_project_moves(&path, "save")?;
        let tab = self
            .document_state
            .tabs
            .iter()
            .find(|tab| matches!(tab.kind, TabKind::Pcb) && tab.path == path)
            .with_context(|| format!("no open PCB at {}", path.display()))?;
        if !tab.dirty && !self.document_state.dirty_paths.contains(&path) {
            return Ok(None);
        }
        let revision = self
            .document_state
            .pcb_revisions
            .get(&path)
            .copied()
            .unwrap_or(0);
        let board = tab
            .cached_document
            .as_ref()
            .and_then(|doc| doc.as_pcb())
            .with_context(|| {
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
        let job = self
            .document_state
            .document_saves
            .begin(path.clone(), bytes);
        // Net colours are a schematic concern; a board has none, so 0.
        Ok(Some(Self::spawn_document_write(
            job, path, revision, 0, None,
        )))
    }

    fn spawn_document_write(
        job: crate::persist::DocumentSaveJob,
        target: PathBuf,
        revision: u64,
        net_color_revision: u64,
        save_as_from: Option<PathBuf>,
    ) -> Task<Message> {
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
                    net_color_revision,
                    save_as_from: save_as_from.clone(),
                    result,
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
            net_color_revision,
            save_as_from,
            result,
        } = done;
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
            return Task::none();
        }
        self.document_state.document_saves.clear_failure(&path);
        // The real file is now at least as new as any recovery copy of
        // the document it was saved from (or over).
        let recovery_keys = [Some(path.clone()), save_as_from.clone()];
        match save_as_from {
            None if is_pcb_path(&path) => self.mark_pcb_saved(&path, revision),
            None => self.mark_saved(&path, revision, net_color_revision),
            Some(from) => self.apply_saved_as(&from, path, revision, net_color_revision),
        }
        self.warn_unwritten_spellings(&outcome_path);
        self.refresh_panel_ctx();
        Task::batch(
            recovery_keys
                .iter()
                .flatten()
                .map(|key| self.autosave_after_save(key))
                .collect::<Vec<_>>(),
        )
    }

    /// Once no write to the file `path` names (under any spelling) is still owed, warn about
    /// every spelling whose snapshot lost the ordering race and is still
    /// modified: a sibling spelling's newer bytes are on disk instead of its
    /// own, so it must be saved again. A same-document rapid Ctrl+S loser is
    /// not listed: its winner's completion has cleaned it by now. Runs from
    /// both completions, so the warning fires in either completion order.
    fn warn_unwritten_spellings(&mut self, path: &std::path::Path) {
        for path in self.document_state.document_saves.take_superseded(path) {
            if self.document_state.dirty_paths.contains(&path) {
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
        net_color_revision: u64,
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
            return;
        }
        // A colour changed while the write ran: the file lacks it, so the
        // sheet stays dirty and the next save writes it.
        if self.net_color_revision_of(path) != net_color_revision {
            return;
        }
        self.finish_clean_save(path);
    }

    /// A PCB write landed. Like `mark_saved`, a changed board revision means
    /// the bytes are stale: stay dirty and queue no "Save" commit. Routed by
    /// path, so a tab closed mid-write (no tab left) still commits.
    fn mark_pcb_saved(&mut self, path: &std::path::Path, revision: u64) {
        crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
        let tab_open = self.document_state.tabs.iter().any(|tab| tab.path == path);
        // A missing key reads as 0 on both sides (begin uses the same
        // fallback), so it can never pin a tab dirty.
        let current = self
            .document_state
            .pcb_revisions
            .get(path)
            .copied()
            .unwrap_or(0);
        if tab_open && current != revision {
            return;
        }
        self.finish_clean_save(path);
    }

    /// The bytes for `path` landed and match the document: clear its dirty
    /// markers, release a parked engine nothing holds open, and queue the
    /// project git commit.
    fn finish_clean_save(&mut self, path: &std::path::Path) {
        self.document_state.dirty_paths.remove(path);
        for tab in self
            .document_state
            .tabs
            .iter_mut()
            .filter(|t| t.path == path)
        {
            tab.dirty = false;
        }
        // The tab closed mid-save and parked the engine because it was
        // dirty; now it is clean nothing needs it.
        if !self.document_is_held_open(path) {
            self.forget_net_colors_of(path);
            self.document_state.remove_engine(path);
        }
        let label = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        self.commit_save_to_project_git(path, &format!("Save {label}"));
    }

    pub(crate) fn document_is_held_open(&self, path: &std::path::Path) -> bool {
        self.document_state.tabs.iter().any(|tab| tab.path == path)
            || self.ui_state.windows.values().any(|kind| {
                matches!(kind, crate::app::state::WindowKind::UndockedTab { path: p, .. } if p == path)
            })
    }

    /// Save As landed: re-key and retitle the tab, but only if it is still
    /// the schematic the dialog was opened for.
    fn apply_saved_as(
        &mut self,
        from: &std::path::Path,
        to: PathBuf,
        revision: u64,
        net_color_revision: u64,
    ) {
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
            return;
        }
        let renamed = self.with_active_schematic_session_mut(|session| {
            session.finish_save_as(to.clone(), revision);
        });
        if renamed.is_some() {
            let colours_moved_on = self.net_color_revision_of(from) != net_color_revision;
            if let Some(count) = self.ui_state.net_color_revision.remove(from) {
                self.ui_state.net_color_revision.insert(to.clone(), count);
            }
            if colours_moved_on {
                self.mark_cached_sheet_dirty(&to);
                self.refresh_panel_ctx();
            }
            crate::diagnostics::log_info(format!("[save-as] Wrote {}", to.display()));
        }
    }
}

/// Routed by extension, not by an open tab, so a PCB whose tab closed
/// mid-write still logs and commits.
pub(super) fn is_pcb_path(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext == "snxpcb" || ext == "standard_pcb")
}
