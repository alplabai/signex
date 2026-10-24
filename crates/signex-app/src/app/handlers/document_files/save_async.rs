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
        let engine = self
            .document_state
            .engines
            .get(&source)
            .with_context(|| format!("no open schematic at {}", source.display()))?;
        let revision = engine.revision();
        let bytes = engine
            .serialize()
            .with_context(|| format!("encode schematic {}", source.display()))?
            .into_bytes();
        let job = self
            .document_state
            .document_saves
            .begin(target.clone(), bytes);
        Ok(Task::perform(
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
                    result,
                }))
            },
        ))
    }

    /// A schematic write finished. Success clears the dirty marker only if
    /// the document is unchanged since the snapshot; the project git commit
    /// is queued here, after the file is on disk.
    pub(crate) fn handle_document_saved(&mut self, done: DocumentSaveDone) -> Task<Message> {
        self.document_state.document_saves.finish(&done.path);
        let DocumentSaveDone {
            path,
            revision,
            save_as_from,
            result,
        } = done;
        let reason = match result {
            Ok(SaveOutcome::Written) => None,
            // A newer snapshot of this path reached disk first; its own
            // completion decides dirty/git. Nothing of ours landed.
            Ok(SaveOutcome::Superseded) if save_as_from.is_none() => return Task::none(),
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
            self.report_save_failure(context, &anyhow::anyhow!(reason));
            return Task::none();
        }
        match save_as_from {
            None => self.mark_saved(&path, revision),
            Some(from) => self.apply_saved_as(&from, path, revision),
        }
        self.refresh_panel_ctx();
        Task::none()
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
                Ok(()) => Message::Noop,
                Err(reason) => Message::File(FileMsg::DocumentSaveWaitFailed(reason)),
            },
        )
        .chain(next)
    }

    fn mark_saved(&mut self, path: &std::path::Path, revision: u64) {
        crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
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
            self.document_state.engines.remove(path);
        }
        let label = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        self.commit_save_to_project_git(path, &format!("Save {label}"));
    }

    fn document_is_held_open(&self, path: &std::path::Path) -> bool {
        self.document_state.tabs.iter().any(|tab| tab.path == path)
            || self.ui_state.windows.values().any(|kind| {
                matches!(kind, crate::app::state::WindowKind::UndockedTab { path: p, .. } if p == path)
            })
    }

    /// Save As landed: re-key and retitle the tab, but only if it is still
    /// the schematic the dialog was opened for.
    fn apply_saved_as(&mut self, from: &std::path::Path, to: PathBuf, revision: u64) {
        let still_active = matches!(
            self.document_state.tabs.get(self.document_state.active_tab),
            Some(tab) if matches!(tab.kind, TabKind::Schematic) && tab.path == from
        );
        if !still_active || self.save_as_target_is_other_open_document(&to) {
            self.report_save_failure(
                "Failed to save active document as",
                &anyhow::anyhow!(
                    "{} was written, but the tab was switched or the target was opened \
                     meanwhile, so it stays at {}",
                    to.display(),
                    from.display()
                ),
            );
            return;
        }
        let renamed = self.with_active_schematic_session_mut(|session| {
            session.finish_save_as(to.clone(), revision);
        });
        if renamed.is_some() {
            crate::diagnostics::log_info(format!("[save-as] Wrote {}", to.display()));
        }
    }
}
