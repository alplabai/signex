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

/// How a finished primitive write brings the library caches and open
/// previews up to date.
enum PostSaveRefresh<'a> {
    /// Off the UI thread; the tasks are collected for the caller to run.
    Background(&'a mut Vec<Task<Message>>),
    /// On this thread, for the save that is blocking by design and has no
    /// task to return.
    Inline,
}

/// How long the blocking save waits for the reads queued ahead of it.
const BLOCKING_SAVE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

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
        self.refuse_save_while_restoring(path)?;
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
        self.refuse_save_while_restoring(path)?;
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
        self.save_primitive_tab_blocking_within(path, BLOCKING_SAVE_WAIT)
    }

    /// [`Self::save_primitive_tab_blocking`] waiting at most `limit` for the
    /// reads ahead of it. A save that cannot get its turn in time is refused,
    /// never written out of order (#1756).
    pub(crate) fn save_primitive_tab_blocking_within(
        &mut self,
        path: &std::path::Path,
        limit: std::time::Duration,
    ) -> anyhow::Result<()> {
        // This write runs on the UI thread, so it cannot wait for a library
        // write a worker still has: refuse instead of freezing the window.
        if let Some(library) = self
            .library
            .write_target_library(path, self.identity_fold_for(path))
            && self.library.writes.pending_writes(&library.root) > 0
        {
            anyhow::bail!(
                "{}: a library write is still running; try again once it finishes",
                path.display()
            );
        }
        let (write, generation, guard) =
            self.prepare_primitive_save(path, ContainerWrite::Merge)?;
        let result = {
            let _in_flight = guard;
            write.run_within(limit)
        };
        let failure = result.as_ref().err().cloned();
        // This save covers whatever a follow-up would have written, and an
        // unrun follow-up task would leave the path marked as saving.
        self.document_state.document_saves.discard_follow_up(path);
        let done = PrimitiveSaveDone {
            path: path.to_path_buf(),
            generation,
            save_as: None,
            result,
        };
        // Nothing is queued, so there is no follow-up task to run: settle
        // without the follow-up step instead of discarding a `Task`.
        self.settle_primitive_save(done);
        if failure.is_none() {
            // No async completion will clear the bookkeeping, so a quit
            // right after this save must not find a stale recovery copy;
            // callers chain `autosave_sweep` to remove it on disk. Only
            // supersede a running write: `saved` would also drop the path
            // from the written set the sweep reads, leaving the sidecar.
            self.document_state.autosave.supersede_in_flight(path);
        }
        failure.map_or(Ok(()), |reason| Err(anyhow::anyhow!(reason)))
    }

    /// Snapshot the editor and register the write: the pieces of one
    /// container save, ready to run.
    fn prepare_primitive_save(
        &mut self,
        path: &std::path::Path,
        mode: ContainerWrite,
    ) -> anyhow::Result<(PrimitiveWrite, u64, crate::persist::InFlight)> {
        // A rename or delete still moving this file would recreate it (#859).
        self.refuse_while_path_op(path, "saved")?;
        self.refuse_save_while_restoring(path)?;
        // A write into a project whose folder is being renamed races the
        // move: it can land in the old folder before the re-key and never
        // reach the new one.
        self.refuse_while_project_moves(path, "save")?;
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
        let fold = self.identity_fold_for(path);
        let target = self.library.write_target_library(path, fold);
        let mounted_library = target.map(|lib| lib.root.clone());
        let mounted_alias = alias_under(path, target);
        if mounted_library.is_some() && self.library.libraries_share_folder_of(path, fold) {
            crate::diagnostics::log_warning(format!(
                "{}: several libraries share its folder; saved through {}.",
                path.display(),
                self.library
                    .library_label(mounted_library.as_deref().unwrap_or(path)),
            ));
        }
        let turn = mounted_library
            .as_deref()
            .map(|library| self.library.writes.turn(library));
        let others = self.other_library_turns(mounted_library.as_deref(), mode);
        let write = PrimitiveWrite {
            path: path.to_path_buf(),
            mounted_alias,
            mode,
            mounted_library,
            now,
            snapshot,
            turn,
            others,
        };
        let guard = self
            .document_state
            .document_saves
            .begin_work(path.to_path_buf());
        Ok((write, generation, guard))
    }

    /// Stamp the changed primitives and the save `generation`, then clone
    /// the editor's container. Clears the editor's dirty flag and makes the
    /// snapshot's undo depth its save point: any edit from here on sets the
    /// flag again, and Undo back to the depth clears it (#1854).
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
            // A retry with no edit writes nothing, so it is not a change.
            if !changed.is_empty() {
                editor.file.updated = now;
            }
            editor.mark_saved();
            editor.save_generation = generation;
            let snapshot = Snapshot::Symbol {
                file: editor.file.clone(),
                baseline: editor.baseline.clone(),
                name: editor.primitive().name.clone(),
                owed: with_kept(&editor.cascade_owed, self.library.owed_cascades.of(path)),
            };
            self.document_state.keep_document_dirty_until_saved(path);
            return Ok(snapshot);
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
            if !changed.is_empty() {
                editor.file.updated = now;
            }
            editor.mark_saved();
            editor.save_generation = generation;
            let snapshot = Snapshot::Footprint {
                file: editor.file.clone(),
                baseline: editor.baseline.clone(),
                name: editor.primitive().name.clone(),
                owed: with_kept(&editor.cascade_owed, self.library.owed_cascades.of(path)),
            };
            self.document_state.keep_document_dirty_until_saved(path);
            return Ok(snapshot);
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
        let path = done.path.clone();
        let written = done.result.is_ok();
        let mut tasks = Vec::new();
        self.settle_primitive_save_state(done, PostSaveRefresh::Background(&mut tasks));
        tasks.push(self.start_follow_up_save(&path));
        self.refresh_panel_ctx();
        if written {
            // The container is on disk: its recovery copy is stale (#1185).
            // An edit made while the write ran is copied again by the next
            // tick.
            tasks.push(self.autosave_after_save(&path));
        }
        Task::batch(tasks)
    }

    /// Settle a finished write that has no follow-up to start: the
    /// synchronous save paths use this, so no `Task` is produced to drop.
    fn settle_primitive_save(&mut self, done: PrimitiveSaveDone) {
        self.settle_primitive_save_state(done, PostSaveRefresh::Inline);
        self.refresh_panel_ctx();
    }

    /// The state half of a completion: release the path, then apply the
    /// success or failure to the draft.
    fn settle_primitive_save_state(
        &mut self,
        done: PrimitiveSaveDone,
        refresh: PostSaveRefresh<'_>,
    ) {
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
                self.finish_primitive_save(&path, generation, written, save_as.is_some(), refresh);
            }
            Err(reason) => self.fail_primitive_save(&path, generation, save_as, reason),
        }
        // The write's turn is released and the draft settled (a draft with a
        // save in flight counts as unsaved): a close that waited for it is
        // due, and a failed write holds it.
        self.close_libraries_whose_writes_are_done();
    }

    /// Run the saves queued while the write to `path` (or a case variant of
    /// its name) was running, each for the spelling that asked and only if
    /// that editor is still the one that asked and still has something to
    /// write. Only one starts now; the others find the file busy and queue
    /// again, so they run one after another as each write completes.
    fn start_follow_up_save(&mut self, path: &std::path::Path) -> Task<Message> {
        if self.document_state.document_saves.is_saving(path) {
            return Task::none();
        }
        let asked = self.document_state.document_saves.take_follow_ups(path);
        let mut started = Vec::new();
        for (asking, generation) in asked {
            // Only the editor that asked may be saved: one that replaced it
            // since holds a draft nobody asked to save.
            if self.primitive_save_generation(&asking) != Some(generation)
                || !self.document_state.is_document_dirty(&asking)
            {
                continue;
            }
            match self.save_primitive_tab_at(&asking) {
                Ok(task) => started.push(task),
                Err(error) => self.report_save_failure("Save failed", &error),
            }
        }
        Task::batch(started)
    }

    fn finish_primitive_save(
        &mut self,
        path: &std::path::Path,
        generation: u64,
        written: PrimitiveWritten,
        was_save_as: bool,
        refresh: PostSaveRefresh<'_>,
    ) {
        let label = if was_save_as { "save-as" } else { "save" };
        // The picker and open Component Previews read the new bytes. Rows
        // the cascade moved changed under their open previews: clean ones
        // reload, dirty ones keep their draft and are refused as stale on
        // save (#1072).
        // The refresh follows the library the worker wrote through, which
        // is not always the one the UI thread picked (#1972): an open one
        // is refreshed as the cascade's own, an unmounted one has no cache.
        let elsewhere = self.written_elsewhere(path, written.library.as_deref());
        match refresh {
            PostSaveRefresh::Background(tasks) => {
                match &elsewhere {
                    None => {
                        tasks.push(self.refresh_primitive_cache_for(path));
                        tasks
                            .push(self.refresh_editors_after_cascade(path, &written.cascaded_rows));
                    }
                    Some(root) if self.library.library_at(root).is_some() => {
                        tasks.push(self.refresh_cascaded_library(root, &written.cascaded_rows));
                    }
                    Some(_) => {}
                }
                for (root, rows) in &written.cascaded_elsewhere {
                    tasks.push(self.refresh_cascaded_library(root, rows));
                }
            }
            PostSaveRefresh::Inline => {
                match &elsewhere {
                    None => {
                        self.refresh_primitive_cache_blocking(path);
                        self.refresh_editors_after_cascade_blocking(path, &written.cascaded_rows);
                    }
                    Some(root) if self.library.library_at(root).is_some() => {
                        self.refresh_cascaded_library_blocking(root, &written.cascaded_rows);
                    }
                    Some(_) => {}
                }
                for (root, rows) in &written.cascaded_elsewhere {
                    self.refresh_cascaded_library_blocking(root, rows);
                }
            }
        }
        // The editor the snapshot came from is gone: closed, discarded and
        // reopened, or History-restored while the write ran. The bytes
        // landed, but its baseline, versions and dirty state are not this
        // save's to set, and "Save X" would mislabel what is committed.
        let edited_in_place = self.primitive_save_generation(path) == Some(generation);
        // What the write still owes follows the file, not the editor.
        self.library.owed_cascades.set(
            path,
            if edited_in_place {
                &[]
            } else {
                &written.cascade_owed
            },
        );
        if !edited_in_place {
            crate::diagnostics::log_info(format!(
                "[{label}] Wrote {}; the editor it was saved from is no longer open, \
                 so the open document was left as it is",
                path.display()
            ));
            self.pay_debt_of_replacement(path, &written.cascade_owed);
        } else {
            self.settle_saved_editor(path, written.file, &written.versions, &written.cascade_owed);
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

    /// The library a write went through when it is not the one this app
    /// would pick for `path` (`LibraryState::write_target_library`).
    fn written_elsewhere(
        &self,
        path: &std::path::Path,
        through: Option<&std::path::Path>,
    ) -> Option<std::path::PathBuf> {
        let through = through?;
        let picked = self.library_root_containing(path);
        (picked.as_deref() != Some(through)).then(|| through.to_path_buf())
    }

    /// The editor the snapshot came from takes the written baseline and
    /// versions, and is marked clean unless it was edited meanwhile.
    fn settle_saved_editor(
        &mut self,
        path: &std::path::Path,
        file: WrittenContainer,
        versions: &std::collections::HashMap<uuid::Uuid, String>,
        owed: &[uuid::Uuid],
    ) {
        let edited_since = match file {
            WrittenContainer::Symbol(saved) => self
                .document_state
                .symbol_editors
                .get_mut(path)
                .map(|editor| {
                    cascade::apply_symbol_versions(&mut editor.file, versions);
                    editor.baseline = saved;
                    editor.cascade_owed = owed.to_vec();
                    // Older undo steps still hold the pre-bump version, so
                    // the saved depth no longer reproduces the file.
                    if editor.is_dirty() && !versions.is_empty() {
                        editor.history.forget();
                    }
                    editor.is_dirty()
                }),
            WrittenContainer::Footprint(saved) => self
                .document_state
                .footprint_editors
                .get_mut(path)
                .map(|editor| {
                    cascade::apply_footprint_versions(&mut editor.file, versions);
                    editor.baseline = saved;
                    editor.cascade_owed = owed.to_vec();
                    if editor.is_dirty() && !versions.is_empty() {
                        editor.history.forget();
                    }
                    editor.is_dirty()
                }),
        };
        // Rows a failed cascade left behind are an unsaved obligation: the
        // tab dot, the close prompt and the quit guard cover it, and the
        // prompt's Save is the retry.
        match edited_since {
            Some(_) if !owed.is_empty() => self.mark_primitive_owes_cascade(path),
            Some(false) => self.mark_primitive_clean(path),
            Some(true) => self.document_state.release_save_hold(path),
            None => {}
        }
    }

    /// The write finished for an editor that was replaced meanwhile. If the
    /// replacement was handed the debt this write paid ([`Self::seed_owed_cascade`]),
    /// drop what is paid, and let it read clean when that debt was all that
    /// kept it unsaved. Edits it holds stay unsaved (#2004).
    fn pay_debt_of_replacement(&mut self, path: &std::path::Path, still_owed: &[uuid::Uuid]) {
        let document = &mut self.document_state;
        let settled = if let Some(editor) = document.symbol_editors.get_mut(path) {
            let owed = !editor.cascade_owed.is_empty();
            editor.cascade_owed.retain(|id| still_owed.contains(id));
            owed && editor.cascade_owed.is_empty()
                && editor.file.changed_since(&editor.baseline).is_empty()
        } else if let Some(editor) = document.footprint_editors.get_mut(path) {
            let owed = !editor.cascade_owed.is_empty();
            editor.cascade_owed.retain(|id| still_owed.contains(id));
            owed && editor.cascade_owed.is_empty()
                && editor.file.changed_since(&editor.baseline).is_empty()
        } else {
            false
        };
        if settled {
            self.mark_primitive_clean(path);
        }
    }

    /// An editor that owes row updates is closed: the file keeps owing them,
    /// and its next save carries them out (#2004).
    pub(crate) fn keep_owed_cascade(&mut self, path: &std::path::Path) {
        let document = &self.document_state;
        let owed = document
            .symbol_editors
            .get(path)
            .map(|e| e.cascade_owed.clone())
            .or_else(|| {
                document
                    .footprint_editors
                    .get(path)
                    .map(|e| e.cascade_owed.clone())
            })
            .unwrap_or_default();
        if owed.is_empty() {
            return;
        }
        self.library.owed_cascades.set(path, &owed);
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let line = format!(
            "Rows that use {name} still await their update; the next save of it moves them, \
             but only while Signex stays open"
        );
        crate::diagnostics::log_warning(crate::text_trunc::prefix_within_bytes(&line, 157));
    }

    /// At quit: name the files whose rows still await an update, since the
    /// record does not survive the process (#2004).
    pub(crate) fn warn_owed_cascades_at_quit(&self) {
        let document = &self.document_state;
        let mut files: Vec<&std::path::Path> = self.library.owed_cascades.files().collect();
        files.extend(
            document
                .symbol_editors
                .iter()
                .filter(|(_, e)| !e.cascade_owed.is_empty())
                .map(|(path, _)| path.as_path()),
        );
        files.extend(
            document
                .footprint_editors
                .iter()
                .filter(|(_, e)| !e.cascade_owed.is_empty())
                .map(|(path, _)| path.as_path()),
        );
        files.sort();
        files.dedup();
        if files.is_empty() {
            return;
        }
        let names: Vec<String> = files
            .iter()
            .map(|p| {
                p.file_stem().map_or_else(
                    || p.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                )
            })
            .collect();
        let line = format!(
            "Quit with rows still awaiting an update from: {}; save those files again to move them",
            names.join(", ")
        );
        crate::diagnostics::log_warning(crate::text_trunc::prefix_within_bytes(&line, 157));
    }

    /// A reopened editor takes back the row updates its file still owes, and
    /// is unsaved until a save moves them (#2004).
    pub(crate) fn seed_owed_cascade(&mut self, path: &std::path::Path) {
        let owed = self.library.owed_cascades.take(path);
        if owed.is_empty() {
            return;
        }
        let document = &mut self.document_state;
        if let Some(editor) = document.symbol_editors.get_mut(path) {
            editor.cascade_owed = owed;
        } else if let Some(editor) = document.footprint_editors.get_mut(path) {
            editor.cascade_owed = owed;
        } else {
            self.library.owed_cascades.set(path, &owed);
            return;
        }
        self.mark_primitive_owes_cascade(path);
    }

    /// Say so when a History restore replaces an editor that still owes row
    /// updates: the restored file is not the one they were owed for, so they
    /// are dropped and the user has to re-bind by hand. (Closing an editor
    /// keeps the debt instead: [`Self::keep_owed_cascade`].)
    pub(crate) fn warn_if_cascade_owed(&self, path: &std::path::Path) {
        let document = &self.document_state;
        let owes = document
            .symbol_editors
            .get(path)
            .map(|e| !e.cascade_owed.is_empty())
            .or_else(|| {
                document
                    .footprint_editors
                    .get(path)
                    .map(|e| !e.cascade_owed.is_empty())
            })
            .unwrap_or(false);
        if !owes {
            return;
        }
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        // A restore replaces the file, so what the old editor owed no longer
        // describes it: unlike a close, nothing is kept (#2004).
        let line = format!(
            "{name} was restored from history, so its pending row updates are void; \
             re-bind the rows that use it by hand"
        );
        crate::diagnostics::log_warning(crate::text_trunc::prefix_within_bytes(&line, 157));
    }

    /// The file at `path` is saved but its rows are not all moved: keep the
    /// draft marked unsaved.
    fn mark_primitive_owes_cascade(&mut self, path: &std::path::Path) {
        self.document_state.mark_document_dirty(path);
        // The editor flag carries the dirt now; the snapshot's hold is done.
        self.document_state.release_save_hold(path);
    }

    /// The draft at `path` matches disk: drop its dirty markers, and the
    /// editor itself when it was closed while dirty and parked for Save All.
    fn mark_primitive_clean(&mut self, path: &std::path::Path) {
        self.document_state.mark_landed_draft_clean(path);
        if !self.document_is_held_open(path) {
            self.document_state.symbol_editors.remove(path);
            self.document_state.footprint_editors.remove(path);
        }
    }

    /// The draft at `path` has no state known to match disk.
    fn forget_draft_save_point(&mut self, path: &std::path::Path) {
        let document = &mut self.document_state;
        if let Some(editor) = document.symbol_editors.get_mut(path) {
            editor.history.forget();
        } else if let Some(editor) = document.footprint_editors.get_mut(path) {
            editor.history.forget();
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
        // The snapshot cleared the flag; the draft is still unsaved. Its
        // save point named content that never reached disk, so Undo back to
        // that depth must not read clean.
        if let Some(draft) = &draft {
            self.forget_draft_save_point(draft);
            self.document_state.mark_document_dirty(draft);
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

/// What the editor owes plus what the file kept after an editor of it closed.
fn with_kept(editor: &[uuid::Uuid], kept: &[uuid::Uuid]) -> Vec<uuid::Uuid> {
    let mut all = editor.to_vec();
    all.extend(kept.iter().filter(|id| !editor.contains(id)));
    all
}

/// `path` spelled under the mounted library's own folder, when the library
/// holds it under another spelling (`..`, case). Only a guess from the text:
/// the worker checks on disk that it is the same directory
/// (`PrimitiveWrite::route`) before writing through it.
fn alias_under(
    path: &std::path::Path,
    library: Option<&crate::library::state::OpenLibrary>,
) -> Option<std::path::PathBuf> {
    let dir = library?.root_dir()?;
    (!path.starts_with(dir)).then(|| crate::app::state::scope::rebase_under(path, dir, dir))
}
