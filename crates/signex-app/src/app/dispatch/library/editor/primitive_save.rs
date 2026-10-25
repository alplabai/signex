//! Off-thread symbol / footprint container saves (#859).
//!
//! `update` stamps and snapshots the editor's container and returns a
//! `Task`; the blocking part (the merge into the file on disk, the library
//! commit, the version cascade) runs on a worker. The draft is not marked
//! clean until the completion arrives: the editor's own `dirty` flag is
//! cleared when the snapshot is taken, so an edit made while the write ran
//! sets it again and the draft stays dirty. A failed write sets it back.
//! The quit barrier waits for the write through [`crate::persist::InFlight`].

use super::cascade::{self, SaveLibrary, SavedKind};
use super::container_save::{self, ContainerWrite, SaveOptions};
use super::*;
use crate::persist::{PrimitiveSaveDone, PrimitiveWritten, SaveAsOrigin, WrittenContainer};
use signex_library::{FootprintFile, SymbolFile};

/// A container as the editor held it when the save began.
enum Snapshot {
    Symbol {
        file: SymbolFile,
        baseline: SymbolFile,
        name: String,
    },
    Footprint {
        file: FootprintFile,
        baseline: FootprintFile,
        name: String,
    },
}

/// One blocking container write. Run it off the UI thread.
struct PrimitiveWrite {
    path: std::path::PathBuf,
    mode: ContainerWrite,
    /// The `.snxlib` of the open library whose directory holds `path`.
    mounted_library: Option<std::path::PathBuf>,
    now: chrono::DateTime<chrono::Utc>,
    snapshot: Snapshot,
}

impl PrimitiveWrite {
    fn run(self) -> Result<PrimitiveWritten, String> {
        let Self {
            path,
            mode,
            mounted_library,
            now,
            snapshot,
        } = self;
        let library = SaveLibrary::resolve(&path, mounted_library.as_deref());
        let options = SaveOptions {
            mode,
            bump_versions: library.bumps_versions(mode),
        };
        // The reason leads: the Messages panel cuts long lines.
        let failed = |e: signex_library::LibraryError| format!("{e} (writing {})", path.display());
        match snapshot {
            Snapshot::Symbol {
                mut file,
                baseline,
                name,
            } => {
                let save = container_save::save_symbol_container(
                    &library,
                    &path,
                    &file,
                    &baseline,
                    now,
                    options,
                    &format!("save symbol {name}"),
                )
                .map_err(failed)?;
                // A symbol this editor deleted is named from the baseline.
                let (mut replaced, mut deleted) = (Vec::new(), Vec::new());
                for uuid in &save.merge.overwritten {
                    match file.get_symbol(*uuid) {
                        Some(symbol) => replaced.push(symbol.name.clone()),
                        None => deleted.extend(baseline.get_symbol(*uuid).map(|s| s.name.clone())),
                    }
                }
                container_save::report_overwritten(&path, &replaced, &deleted);
                // Carry the versions the save bumped into the copy, then
                // move the rows that use them onto the new version (#832),
                // after the library lock was released.
                cascade::apply_symbol_versions(&mut file, &save.versions);
                cascade::cascade_after_container_save(
                    &library,
                    mode,
                    SavedKind::Symbol,
                    &cascade::saved_symbols(&file, &save.merge.changed),
                );
                Ok(PrimitiveWritten {
                    file: WrittenContainer::Symbol(file),
                    versions: save.versions,
                    commit_message: format!("Save symbol {name}"),
                })
            }
            Snapshot::Footprint {
                mut file,
                baseline,
                name,
            } => {
                let save = container_save::save_footprint_container(
                    &library,
                    &path,
                    &file,
                    &baseline,
                    now,
                    options,
                    &format!("save footprint {name}"),
                )
                .map_err(failed)?;
                let name_in = |container: &FootprintFile, uuid: uuid::Uuid| {
                    container
                        .footprints
                        .iter()
                        .find(|f| f.uuid == uuid)
                        .map(|f| f.name.clone())
                };
                let (mut replaced, mut deleted) = (Vec::new(), Vec::new());
                for uuid in &save.merge.overwritten {
                    match name_in(&file, *uuid) {
                        Some(name) => replaced.push(name),
                        None => deleted.extend(name_in(&baseline, *uuid)),
                    }
                }
                container_save::report_overwritten(&path, &replaced, &deleted);
                cascade::apply_footprint_versions(&mut file, &save.versions);
                cascade::cascade_after_container_save(
                    &library,
                    mode,
                    SavedKind::Footprint,
                    &cascade::saved_footprints(&file, &save.merge.changed),
                );
                Ok(PrimitiveWritten {
                    file: WrittenContainer::Footprint(file),
                    versions: save.versions,
                    commit_message: format!("Save footprint {name}"),
                })
            }
        }
    }
}

impl Signex {
    /// Write the primitive at `path` back to disk as JSON: snapshot the
    /// editor now, return the task that does the blocking write. The
    /// completion ([`Self::handle_primitive_saved`]) marks the draft clean,
    /// queues the project git commit and refreshes the library caches.
    ///
    /// Errors name WHY nothing was started (no editor, a write to the path
    /// still running) so the caller can surface them.
    pub(crate) fn save_primitive_tab_at(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<Task<Message>> {
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
        self.begin_primitive_save(path, ContainerWrite::Replace, Some(origin))
    }

    fn begin_primitive_save(
        &mut self,
        path: &std::path::Path,
        mode: ContainerWrite,
        save_as: Option<SaveAsOrigin>,
    ) -> anyhow::Result<Task<Message>> {
        if self.document_state.document_saves.is_saving(path) {
            anyhow::bail!(
                "{}: a save is still running; try again once it finishes",
                path.display()
            );
        }
        let now = chrono::Utc::now();
        let snapshot = self.snapshot_primitive(path, now)?;
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
                    save_as: save_as.clone(),
                    result,
                })))
            },
        ))
    }

    /// Stamp the changed primitives and clone the editor's container. Clears
    /// the editor's dirty flag: any edit from here on sets it again.
    fn snapshot_primitive(
        &mut self,
        path: &std::path::Path,
        now: chrono::DateTime<chrono::Utc>,
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
            return Ok(Snapshot::Symbol {
                file: editor.file.clone(),
                baseline: editor.baseline.clone(),
                name: editor.primitive().name.clone(),
            });
        }
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
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

    /// A container write finished. Success clears the draft's dirty markers
    /// unless it was edited meanwhile; failure puts the dirty flag back (and
    /// undoes a Save As re-key) and reaches the Messages panel and the Save
    /// Failed card.
    pub(crate) fn handle_primitive_saved(&mut self, done: PrimitiveSaveDone) -> Task<Message> {
        self.document_state.document_saves.finish(&done.path);
        let PrimitiveSaveDone {
            path,
            save_as,
            result,
        } = done;
        match result {
            Ok(written) => {
                self.document_state.document_saves.clear_failure(&path);
                self.finish_primitive_save(&path, written, save_as.is_some());
            }
            Err(reason) => self.fail_primitive_save(&path, save_as, reason),
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    fn finish_primitive_save(
        &mut self,
        path: &std::path::Path,
        written: PrimitiveWritten,
        was_save_as: bool,
    ) {
        let PrimitiveWritten {
            file,
            versions,
            commit_message,
        } = written;
        // The editor can be gone: its tab closed while the write ran.
        let edited_since = match file {
            WrittenContainer::Symbol(saved) => self
                .document_state
                .symbol_editors
                .get_mut(path)
                .map(|editor| {
                    cascade::apply_symbol_versions(&mut editor.file, &versions);
                    editor.baseline = saved;
                    editor.dirty
                }),
            WrittenContainer::Footprint(saved) => self
                .document_state
                .footprint_editors
                .get_mut(path)
                .map(|editor| {
                    cascade::apply_footprint_versions(&mut editor.file, &versions);
                    editor.baseline = saved;
                    editor.dirty
                }),
        };
        if edited_since != Some(true) {
            self.document_state.dirty_paths.remove(path);
            for tab in self
                .document_state
                .tabs
                .iter_mut()
                .filter(|tab| tab.path == path)
            {
                tab.dirty = false;
            }
            // Closed while dirty, parked for Save All: nothing needs it now.
            if edited_since.is_some() && !self.document_is_held_open(path) {
                self.document_state.symbol_editors.remove(path);
                self.document_state.footprint_editors.remove(path);
            }
        }
        crate::diagnostics::log_info(format!(
            "[{}] Wrote {}",
            if was_save_as { "save-as" } else { "save" },
            path.display()
        ));
        // Also commit into the owning project's git repo when its
        // `enable_git` is on. The library's own commit happened inside the
        // write, under its lock (#904).
        self.commit_save_to_project_git(path, &commit_message);
        // The picker and open Component Previews read the new bytes.
        self.refresh_primitive_cache_for(path);
        self.reload_primitive_in_library_set(path);
        if was_save_as {
            self.attach_library_for_path(path);
        }
    }

    fn fail_primitive_save(
        &mut self,
        path: &std::path::Path,
        save_as: Option<SaveAsOrigin>,
        reason: String,
    ) {
        let (context, restored, reason) = match save_as {
            Some(origin) => {
                let rolled_back = self.retarget_primitive(path, &origin.from, origin.title);
                let reason = match rolled_back {
                    Ok(_) => format!(
                        "save as {}: write failed, document left at {}: {reason}",
                        path.display(),
                        origin.from.display()
                    ),
                    Err(error) => {
                        format!("{reason}; and the Save As re-key was not undone: {error:#}")
                    }
                };
                ("Save As failed", origin.from, reason)
            }
            None => ("Save failed", path.to_path_buf(), reason),
        };
        // The snapshot cleared the flag; the draft is still unsaved.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(&restored) {
            editor.dirty = true;
        }
        if let Some(editor) = self.document_state.footprint_editors.get_mut(&restored) {
            editor.dirty = true;
        }
        self.document_state
            .document_saves
            .record_failure(restored, reason.clone());
        self.report_save_failure(context, &anyhow::anyhow!(reason));
    }
}
