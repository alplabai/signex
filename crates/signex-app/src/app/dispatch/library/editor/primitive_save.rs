//! Off-thread symbol / footprint container saves (#859).
//!
//! `update` stamps and snapshots the editor's container and returns a
//! `Task`; the blocking part runs on a worker ([`PrimitiveWrite`]). The
//! draft is not marked clean until the completion arrives: the editor's own
//! `dirty` flag is cleared when the snapshot is taken, so an edit made while
//! the write ran sets it again and the draft stays dirty. A failed write
//! sets it back. Each snapshot stamps the editor with a fresh save
//! generation; a completion only touches an editor still wearing it, so a
//! draft discarded and reopened (or History-restored) meanwhile is left
//! alone. Saving a path whose write is still running queues one follow-up
//! save instead. The quit barrier waits for the write through
//! [`crate::persist::InFlight`].

use super::cascade;
use super::container_save::ContainerWrite;
use super::primitive_write::{PrimitiveWrite, Snapshot};
use super::*;
use crate::persist::{PrimitiveSaveDone, PrimitiveWritten, SaveAsOrigin, WrittenContainer};

impl Signex {
    /// Write the primitive at `path` back to disk as JSON: snapshot the
    /// editor now, return the task that does the blocking write. The
    /// completion ([`Self::handle_primitive_saved`]) marks the draft clean,
    /// queues the project git commit and refreshes the library caches.
    ///
    /// While a write to `path` is still running this queues one follow-up
    /// save, run when that write completes, and returns no task. Errors name
    /// WHY nothing was started (no editor) so the caller can surface them.
    pub(crate) fn save_primitive_tab_at(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<Task<Message>> {
        if self.document_state.document_saves.is_saving(path) {
            let Some(asker) = self.primitive_save_generation(path) else {
                anyhow::bail!(
                    "no open editor backs {}; close the tab and reopen it",
                    path.display()
                );
            };
            if self
                .document_state
                .document_saves
                .queue_follow_up(path.to_path_buf(), asker)
            {
                crate::diagnostics::log_info(format!(
                    "[save] {} is still being written; it saves again when that write finishes",
                    path.display()
                ));
            }
            return Ok(Task::none());
        }
        self.begin_primitive_save(path, ContainerWrite::Merge, None)
    }

    /// Save As: write the editor's container to `path` whole, replacing
    /// whatever file the user chose to overwrite there. `origin` is where
    /// the editor was re-keyed from, put back if the write fails.
    pub(crate) fn save_primitive_tab_replacing(
        &mut self,
        path: &std::path::Path,
        origin: SaveAsOrigin,
    ) -> anyhow::Result<Task<Message>> {
        if self.document_state.document_saves.is_saving(path) {
            anyhow::bail!(
                "{}: a save is still running; try again once it finishes",
                path.display()
            );
        }
        self.begin_primitive_save(path, ContainerWrite::Replace, Some(origin))
    }

    fn begin_primitive_save(
        &mut self,
        path: &std::path::Path,
        mode: ContainerWrite,
        save_as: Option<SaveAsOrigin>,
    ) -> anyhow::Result<Task<Message>> {
        let (write, generation, guard) = self.prepare_primitive_save(path, mode)?;
        let target = path.to_path_buf();
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let _in_flight = guard;
                    write.run()
                })
                .await
                .map_err(|join_error| format!("the save task failed: {join_error}"))
                .and_then(|result| result)
            },
            move |result| {
                Message::File(FileMsg::PrimitiveSaved(Box::new(PrimitiveSaveDone {
                    path: target.clone(),
                    generation,
                    save_as: save_as.clone(),
                    result,
                })))
            },
        ))
    }

    /// Save the primitive at `path` now, on this thread, for the flows that
    /// must have the bytes on disk before they go on (the Exclude prompt's
    /// Save). Waits for running writes first, so this one lands last.
    pub(crate) fn save_primitive_tab_blocking(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        self.document_state.document_saves.barrier().wait();
        let (write, generation, guard) =
            self.prepare_primitive_save(path, ContainerWrite::Merge)?;
        let result = {
            let _in_flight = guard;
            write.run()
        };
        let failure = result.as_ref().err().cloned();
        // This save covers whatever a follow-up would have written, and an
        // unrun follow-up task would leave the path marked as saving.
        self.document_state.document_saves.take_follow_up(path);
        let done = PrimitiveSaveDone {
            path: path.to_path_buf(),
            generation,
            save_as: None,
            result,
        };
        // Nothing is queued, so there is no follow-up task to run.
        drop(self.handle_primitive_saved(done));
        failure.map_or(Ok(()), |reason| Err(anyhow::anyhow!(reason)))
    }

    /// Snapshot the editor and register the write: the pieces of one
    /// container save, ready to run.
    fn prepare_primitive_save(
        &mut self,
        path: &std::path::Path,
        mode: ContainerWrite,
    ) -> anyhow::Result<(PrimitiveWrite, u64, crate::persist::InFlight)> {
        // The editors write the container file themselves, bypassing the
        // adapter, so the read-only mount is enforced here (#329).
        if let Some(error) = self.read_only_write_error(path) {
            return Err(error);
        }
        let now = chrono::Utc::now();
        let generation = self.document_state.document_saves.mint_generation();
        let snapshot = self.snapshot_primitive(path, now, generation)?;
        // The container write may add or replace a listed `.snxsym` /
        // `.snxfpt` without changing the path set (#111).
        self.document_state.project_fs.invalidate();
        let mounted_library = self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.root_dir().is_some_and(|d| path.starts_with(d)))
            .map(|lib| lib.root.clone());
        let write = PrimitiveWrite {
            path: path.to_path_buf(),
            mode,
            mounted_library,
            now,
            snapshot,
        };
        let guard = self
            .document_state
            .document_saves
            .begin_work(path.to_path_buf());
        Ok((write, generation, guard))
    }

    /// Stamp the changed primitives and the save `generation`, then clone
    /// the editor's container. Clears the editor's dirty flag: any edit from
    /// here on sets it again.
    fn snapshot_primitive(
        &mut self,
        path: &std::path::Path,
        now: chrono::DateTime<chrono::Utc>,
        generation: u64,
    ) -> anyhow::Result<Snapshot> {
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            // Stamp the changed symbols so downstream consumers can
            // detect the rewrite.
            let changed = editor.file.changed_since(&editor.baseline);
            for symbol in &mut editor.file.symbols {
                if changed.contains(&symbol.uuid) {
                    symbol.updated = now;
                }
            }
            editor.file.updated = now;
            editor.dirty = false;
            editor.save_generation = generation;
            return Ok(Snapshot::Symbol {
                file: editor.file.clone(),
                baseline: editor.baseline.clone(),
                name: editor.primitive().name.clone(),
            });
        }
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
            // A drag solve still owed would be written unsolved, with the
            // pads of the previous solve, and the dirty flag cleared under
            // it: finish it first, rejection included.
            editor.settle_pending_solve();
            // `state.pads` is authoritative on the editor side; without
            // this sync, in-editor pad edits would not persist.
            {
                let (state, primitive) = editor.parts_mut();
                crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(
                    state, primitive,
                );
            }
            // Only the changed footprints are merged into the file on
            // disk, as for symbols (#902).
            let changed = editor.file.changed_since(&editor.baseline);
            for footprint in &mut editor.file.footprints {
                if changed.contains(&footprint.uuid) {
                    footprint.updated = now;
                }
            }
            editor.file.updated = now;
            editor.dirty = false;
            editor.save_generation = generation;
            return Ok(Snapshot::Footprint {
                file: editor.file.clone(),
                baseline: editor.baseline.clone(),
                name: editor.primitive().name.clone(),
            });
        }
        // Reporting success here would log a write that never happened.
        anyhow::bail!(
            "no open editor backs {}; close the tab and reopen it",
            path.display()
        )
    }

    /// The save generation of the editor at `path`, if one is open.
    fn primitive_save_generation(&self, path: &std::path::Path) -> Option<u64> {
        let document = &self.document_state;
        document
            .symbol_editors
            .get(path)
            .map(|editor| editor.save_generation)
            .or_else(|| {
                document
                    .footprint_editors
                    .get(path)
                    .map(|editor| editor.save_generation)
            })
    }

    /// A container write finished. Success clears the draft's dirty markers
    /// unless it was edited meanwhile; failure puts the dirty flag back (and
    /// undoes a Save As re-key) and reaches the Messages panel and the Save
    /// Failed card. Either way a queued follow-up save then starts.
    pub(crate) fn handle_primitive_saved(&mut self, done: PrimitiveSaveDone) -> Task<Message> {
        self.document_state.document_saves.finish(&done.path);
        let PrimitiveSaveDone {
            path,
            generation,
            save_as,
            result,
        } = done;
        match result {
            Ok(written) => {
                self.document_state.document_saves.clear_failure(&path);
                self.finish_primitive_save(&path, generation, written, save_as.is_some());
            }
            Err(reason) => self.fail_primitive_save(&path, generation, save_as, reason),
        }
        let follow_up = self.start_follow_up_save(&path);
        self.refresh_panel_ctx();
        follow_up
    }

    /// Run the save queued while the write to `path` was running, if the
    /// draft still has something to write.
    fn start_follow_up_save(&mut self, path: &std::path::Path) -> Task<Message> {
        let saves = &mut self.document_state.document_saves;
        if saves.is_saving(path) {
            return Task::none();
        }
        let Some(asker) = saves.take_follow_up(path) else {
            return Task::none();
        };
        // Only the editor that asked may be saved: one that replaced it
        // since holds a draft nobody asked to save.
        if self.primitive_save_generation(path) != Some(asker)
            || !self.primitive_draft_is_dirty(path)
        {
            return Task::none();
        }
        match self.save_primitive_tab_at(path) {
            Ok(task) => task,
            Err(error) => {
                self.report_save_failure("Save failed", &error);
                Task::none()
            }
        }
    }

    fn finish_primitive_save(
        &mut self,
        path: &std::path::Path,
        generation: u64,
        written: PrimitiveWritten,
        was_save_as: bool,
    ) {
        let label = if was_save_as { "save-as" } else { "save" };
        // The picker and open Component Previews read the new bytes.
        self.refresh_primitive_cache_for(path);
        self.reload_primitive_in_library_set(path);
        // Rows the cascade moved changed under their open previews: clean
        // ones reload, dirty ones keep their draft and are refused as
        // stale on save (#1072).
        self.refresh_editors_after_cascade(path, &written.cascaded_rows);
        // The editor the snapshot came from is gone: closed, discarded and
        // reopened, or History-restored while the write ran. The bytes
        // landed, but its baseline, versions and dirty state are not this
        // save's to set, and "Save X" would mislabel what is committed.
        let edited_in_place = self.primitive_save_generation(path) == Some(generation);
        if !edited_in_place {
            crate::diagnostics::log_info(format!(
                "[{label}] Wrote {}; the editor it was saved from is no longer open, \
                 so the open document was left as it is",
                path.display()
            ));
        } else {
            self.settle_saved_editor(path, written.file, &written.versions);
            crate::diagnostics::log_info(format!("[{label}] Wrote {}", path.display()));
        }
        // The bytes landed either way: commit them into the owning
        // project's git repo when its `enable_git` is on (the library's own
        // commit happened inside the write, under its lock, #904), and
        // attach the library a Save As wrote into.
        if edited_in_place {
            self.commit_save_to_project_git(path, &written.commit_message);
        } else {
            self.commit_save_to_project_git_if_unqueued(path, &written.commit_message);
        }
        if was_save_as {
            self.attach_library_for_path(path);
        }
    }

    /// The editor the snapshot came from takes the written baseline and
    /// versions, and is marked clean unless it was edited meanwhile.
    fn settle_saved_editor(
        &mut self,
        path: &std::path::Path,
        file: WrittenContainer,
        versions: &std::collections::HashMap<uuid::Uuid, String>,
    ) {
        let edited_since = match file {
            WrittenContainer::Symbol(saved) => self
                .document_state
                .symbol_editors
                .get_mut(path)
                .map(|editor| {
                    cascade::apply_symbol_versions(&mut editor.file, versions);
                    editor.baseline = saved;
                    editor.dirty
                }),
            WrittenContainer::Footprint(saved) => self
                .document_state
                .footprint_editors
                .get_mut(path)
                .map(|editor| {
                    cascade::apply_footprint_versions(&mut editor.file, versions);
                    editor.baseline = saved;
                    editor.dirty
                }),
        };
        if edited_since == Some(false) {
            self.mark_primitive_clean(path);
        }
    }

    /// The draft at `path` matches disk: drop its dirty markers, and the
    /// editor itself when it was closed while dirty and parked for Save All.
    fn mark_primitive_clean(&mut self, path: &std::path::Path) {
        self.document_state.dirty_paths.remove(path);
        for tab in self
            .document_state
            .tabs
            .iter_mut()
            .filter(|tab| tab.path == path)
        {
            tab.dirty = false;
        }
        if !self.document_is_held_open(path) {
            self.document_state.symbol_editors.remove(path);
            self.document_state.footprint_editors.remove(path);
        }
    }

    fn fail_primitive_save(
        &mut self,
        path: &std::path::Path,
        generation: u64,
        save_as: Option<SaveAsOrigin>,
        reason: String,
    ) {
        // Only the editor the snapshot came from holds this unsaved draft.
        let ours = self.primitive_save_generation(path) == Some(generation);
        let (context, draft, reason) = match save_as {
            Some(origin) => {
                let (draft, reason) = self.roll_back_save_as(path, origin, ours, reason);
                ("Save As failed", draft, reason)
            }
            None => ("Save failed", ours.then(|| path.to_path_buf()), reason),
        };
        // The snapshot cleared the flag; the draft is still unsaved.
        if let Some(draft) = &draft {
            if let Some(editor) = self.document_state.symbol_editors.get_mut(draft) {
                editor.dirty = true;
            }
            if let Some(editor) = self.document_state.footprint_editors.get_mut(draft) {
                editor.dirty = true;
            }
            self.document_state.dirty_paths.insert(draft.clone());
        }
        let failed_path = draft.unwrap_or_else(|| path.to_path_buf());
        self.document_state
            .document_saves
            .record_failure(failed_path, reason.clone());
        self.report_save_failure(context, &anyhow::anyhow!(reason));
    }

    /// Undo a failed Save As re-key from `to` back to `origin.from`. Returns
    /// where the unsaved draft now is and the reason to report. When `from`
    /// was reopened while the write ran, the draft stays at `to`: moving it
    /// back would overwrite that editor and leave two tabs on one file.
    fn roll_back_save_as(
        &mut self,
        to: &std::path::Path,
        origin: SaveAsOrigin,
        ours: bool,
        reason: String,
    ) -> (Option<std::path::PathBuf>, String) {
        if !ours {
            return (
                None,
                format!(
                    "save as {}: write failed, and the draft was closed meanwhile: {reason}",
                    to.display()
                ),
            );
        }
        match self.retarget_primitive(to, &origin.from, origin.title) {
            Ok(_) => (
                Some(origin.from.clone()),
                format!(
                    "save as {}: write failed, document left at {}: {reason}",
                    to.display(),
                    origin.from.display()
                ),
            ),
            Err(error) => (
                Some(to.to_path_buf()),
                format!(
                    "save as {}: write failed: {reason}. The draft stays unsaved in its tab \
                     at {} because {error:#}",
                    to.display(),
                    to.display()
                ),
            ),
        }
    }
}
