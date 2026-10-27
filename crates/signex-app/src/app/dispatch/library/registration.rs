//! Library lifecycle handlers — creating a library for a project or
//! at a path, the open-library picker, adding standalone symbol /
//! footprint files, and registering a standalone library onto the
//! active project.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
    /// Resolution of the "New Component Library" save-as dialog — pops
    /// the "Library Options" modal (Git / LFS opt-in) rather than
    /// creating immediately.
    pub(super) fn handle_prompt_library_create_options(
        &mut self,
        project_path: std::path::PathBuf,
        lib_path: std::path::PathBuf,
    ) -> Task<Message> {
        // Stage 11 of `v0.9-snxlib-as-file-plan.md`: pop the
        // "Library Options" modal here instead of creating
        // immediately. The modal lets the user opt into Git
        // LFS for binary 3D models before any disk +
        // `git init` runs. Confirming dispatches
        // `LibraryCreateOptionsConfirm` which calls into
        // `handle_create_library_at_path`.
        self.library.create_options = Some(LibraryCreateOptionsState {
            project_path,
            lib_path,
            enable_git: false,
            use_lfs: false,
        });
        Task::none()
    }

    /// "Library Options" modal — toggle the "Use Git LFS" checkbox.
    pub(super) fn handle_library_create_options_toggle_lfs(&mut self) -> Task<Message> {
        if let Some(state) = self.library.create_options.as_mut() {
            state.use_lfs = !state.use_lfs;
        }
        Task::none()
    }

    /// "Library Options" modal — toggle the "Enable version control"
    /// checkbox.
    pub(super) fn handle_library_create_options_toggle_git(&mut self) -> Task<Message> {
        if let Some(state) = self.library.create_options.as_mut() {
            state.enable_git = !state.enable_git;
            // LFS is meaningless without git — keep the two
            // toggles consistent so the user doesn't end up
            // with LFS-on-no-git which the adapter would
            // silently drop anyway.
            if !state.enable_git {
                state.use_lfs = false;
            }
        }
        Task::none()
    }

    /// "Library Options" modal — Create Library button.
    pub(super) fn handle_library_create_options_confirm(&mut self) -> Task<Message> {
        if let Some(state) = self.library.create_options.take() {
            self.handle_create_library_at_path(
                state.project_path,
                state.lib_path,
                state.enable_git,
                state.use_lfs,
            )
        } else {
            Task::none()
        }
    }

    /// Spawn the "New Component Library" Save-As dialog for the
    /// project rooted at `project_root`. The dialog defaults to
    /// `<project_dir>/<project>-lib.snxlib` so the common
    /// project-local case is one Enter key, but the user can navigate
    /// to a global directory to create a shared library. On confirm,
    /// the dialog dispatches `CreateLibraryAtPath` which calls
    /// `commands::create_library_at` to do the actual disk + manifest
    /// + git init.
    ///
    /// We deliberately do NOT touch disk here — the previous "instant
    /// create on click" behaviour was confusing because users
    /// couldn't see where it was going to land or override the
    /// default name.
    pub(super) fn handle_create_library_for_project(
        &mut self,
        project_root: std::path::PathBuf,
    ) -> Task<Message> {
        // Locate the LoadedProject so we can derive the suggested
        // path. The dispatch handler that consumes the dialog result
        // re-resolves the project at apply time so a project unload
        // between dialog spawn + confirm is recoverable.
        let Some(loaded) =
            self.document_state.projects.iter().find(|p| {
                p.path == project_root || p.path.parent() == Some(project_root.as_path())
            })
        else {
            tracing::warn!(
                target: "signex::library",
                path = %project_root.display(),
                "create library: no loaded project matches root"
            );
            return Task::none();
        };

        let stem = loaded
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("project");
        let mut name = format!("{stem}-lib");

        let project_dir = std::path::PathBuf::from(&loaded.data.dir);
        // Pre-disambiguate the default name so the user doesn't get a
        // misleading "<project>-lib" suggestion when that path already
        // exists. Conflicts get a `-2`, `-3`, … suffix matching the
        // pattern `commands::create_library` previously used.
        if project_dir.join(format!("{name}.snxlib")).exists() {
            for n in 2..=99 {
                let candidate = format!("{stem}-lib-{n}");
                if !project_dir.join(format!("{candidate}.snxlib")).exists() {
                    name = candidate;
                    break;
                }
            }
        }
        let suggested_filename = format!("{name}.snxlib");
        let project_path = loaded.path.clone();

        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("New Component Library")
                    .add_filter("Signex Component Library", &["snxlib"])
                    .set_directory(&project_dir)
                    .set_file_name(&suggested_filename)
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |picked| match picked {
                Some(lib_path) => Message::Library(LibraryMessage::CreateLibraryAtPath {
                    project_path: project_path.clone(),
                    lib_path,
                }),
                None => Message::noop(),
            },
        )
    }

    /// Resolution of the "Library Options" modal (Stage 11 of
    /// `v0.9-snxlib-as-file-plan.md`). Re-resolves the project (in
    /// case it was unloaded between modal spawn + confirm), then
    /// **registers** a pending library — no disk writes here. The
    /// actual `.snxlib/` directory + manifest + git scaffolding land
    /// at project-save time via
    /// `commands::materialize_pending_library`, called from
    /// `save_active_project_if_dirty`. Closes
    /// `feedback_no_disk_writes_without_user_save.md`'s "wait for
    /// explicit user save" invariant. `use_lfs` carries the modal's
    /// checkbox state — when on, the eventual adapter writes
    /// `.gitattributes` for `*.step` / `*.stp` / `*.wrl` / `*.iges`
    /// and stages it into the initial commit.
    pub(super) fn handle_create_library_at_path(
        &mut self,
        project_path: std::path::PathBuf,
        lib_path: std::path::PathBuf,
        enable_git: bool,
        use_lfs: bool,
    ) -> Task<Message> {
        let Some(loaded) = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.path == project_path)
        else {
            tracing::warn!(
                target: "signex::library",
                path = %project_path.display(),
                "create library: project unloaded between dialog spawn and confirm"
            );
            return Task::none();
        };

        // Two pending entries for one path would share a tree key, so the
        // tree could expand or select the wrong row (#855).
        // Compared the way the project's volume does (probed on load, #694),
        // so `Parts.snxlib` and `parts.snxlib` are one library where it
        // ignores case.
        use crate::app::state::scope::path_key_with;
        let fold = loaded.case_fold;
        let key = path_key_with(&lib_path, fold);
        if loaded
            .pending_libraries
            .values()
            .any(|spec| path_key_with(&spec.lib_path, fold) == key)
        {
            crate::diagnostics::log_warning(format!(
                "Library {} is already waiting to be created; save the project to create it.",
                lib_path.display()
            ));
            return Task::none();
        }

        match crate::library::commands::register_pending_library(
            lib_path.clone(),
            enable_git,
            use_lfs,
        ) {
            Ok((library_id, spec)) => {
                loaded.pending_libraries.insert(library_id, spec);
                tracing::info!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library = %lib_path.display(),
                    library_id = %library_id,
                    use_lfs,
                    "registered pending library — disk write deferred to project save"
                );
                // Mark the project file dirty so the user is prompted
                // to persist the new library entry in the `.snxprj`.
                let project_path = loaded.path.clone();
                self.document_state.dirty_paths.insert(project_path);
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library = %lib_path.display(),
                    use_lfs,
                    error = %e,
                    "register_pending_library failed (path validation)"
                );
            }
        }

        self.refresh_panel_ctx();
        Task::none()
    }

    pub(super) fn handle_picker_message(&mut self, msg: PickerMsg) -> Task<Message> {
        if let PickerMsg::PanelSearchChanged(s) = msg {
            self.library.panel_search = s;
            return Task::none();
        }
        let Some(picker) = self.library.picker.as_mut() else {
            return Task::none();
        };
        match msg {
            PickerMsg::PanelSearchChanged(_) => {}
            PickerMsg::FilterChanged(s) => {
                picker.filter = s;
            }
            PickerMsg::SelectComponent(summary) => {
                // `ComponentSummary` carries `row_id` directly in the
                // DBLib model; match against that.
                let path = self
                    .library
                    .open_libraries
                    .iter()
                    .find(|lib| {
                        lib.cached_components
                            .iter()
                            .any(|c| c.row_id == summary.row_id)
                    })
                    .map(|lib| lib.root.clone());
                picker.selected = path.map(|p| (p, summary));
            }
            PickerMsg::PlaceSelected => {
                let Some((path, summary)) = picker.selected.clone() else {
                    return Task::none();
                };
                let Some(table) = self.library.table_of_row(&path, summary.row_id) else {
                    self.library.picker = None;
                    self.report_place_failure(&format!(
                        "{} is no longer in the library; refresh and pick it again.",
                        summary.internal_pn.as_str()
                    ));
                    return Task::none();
                };
                return self.handle_place_library_component(
                    path,
                    table,
                    signex_library::RowId::from_uuid(summary.row_id),
                );
            }
        }
        Task::none()
    }

    /// F34 — Save-As dialog confirmed for a new symbol library file
    /// (`.snxsym`). The user picked the location + filename in the
    /// rfd `save_file()` dialog — that click IS the explicit save
    /// action, so the empty `SymbolFile` is written off the update thread;
    /// [`handle_add_library_file_written`] then registers the path on
    /// `project_id` (the project the dialog was opened for) and opens it as a
    /// clean primitive editor tab (dirty=false). Subsequent edits flow
    /// through the regular `Ctrl+S → save_primitive_tab_at` path.
    pub(crate) fn handle_add_library_symbol_file_picked(
        &mut self,
        project_id: crate::app::state::ProjectId,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("NewSymbol")
            .to_string();
        let symbol = signex_library::Symbol::empty(stem);
        let file = signex_library::SymbolFile::from_symbol(symbol);
        // v0.18.4 — emit TOML envelope (mirror of v0.18.2 .snxfpt).
        let text = file.to_toml_string().map_err(|e| e.to_string());
        self.write_new_library_file(project_id, path, text, "symbol")
    }

    /// F34 — Footprint counterpart to
    /// [`handle_add_library_symbol_file_picked`]. Writes an empty
    /// `FootprintFile` (TOML+TSV envelope), then registers the file as a
    /// project library entry and opens it as a clean primitive editor tab.
    pub(crate) fn handle_add_library_footprint_file_picked(
        &mut self,
        project_id: crate::app::state::ProjectId,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("NewFootprint")
            .to_string();
        let footprint = signex_library::Footprint::empty(stem);
        // v0.18.4 — emit TOML+TSV envelope.
        let file = signex_library::FootprintFile::from_footprint(footprint);
        let text = file.to_toml_string().map_err(|e| e.to_string());
        self.write_new_library_file(project_id, path, text, "footprint")
    }

    fn write_new_library_file(
        &mut self,
        project_id: crate::app::state::ProjectId,
        path: std::path::PathBuf,
        text: Result<String, String>,
        kind: &'static str,
    ) -> Task<Message> {
        let text = match text {
            Ok(t) => t,
            Err(e) => {
                crate::diagnostics::log_warning(format!(
                    "New {kind} library {}: serialize failed: {e}",
                    path.display()
                ));
                return Task::none();
            }
        };
        if self.refuse_with_notice_while_project_moves(
            &path,
            "create",
            crate::app::state::ErrorNotice::create,
        ) {
            return Task::none();
        }
        self.begin_project_write(&path);
        Task::perform(
            {
                let path = path.clone();
                crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool(
                    move || write_library_file(&path, &text),
                )
            },
            move |result| {
                Message::Library(LibraryMessage::AddLibraryFileWritten {
                    project_id,
                    path,
                    result,
                })
            },
        )
    }

    /// Completion of the new-library-file write: a failure goes to the
    /// Messages panel; success registers the file on the project the dialog
    /// was opened for (even if it is no longer the active one) and opens it.
    pub(crate) fn handle_add_library_file_written(
        &mut self,
        project_id: crate::app::state::ProjectId,
        path: std::path::PathBuf,
        result: Result<(), String>,
    ) -> Task<Message> {
        self.end_project_write(&path);
        if let Err(e) = result {
            crate::diagnostics::log_warning(format!("New library file: {e}"));
            return Task::none();
        }
        self.register_standalone_library_on_project(project_id, &path);
        self.handle_open_primitive(path)
    }

    /// Push a `LibraryEntry` for `path` onto `project_id`'s `data.libraries`
    /// (project-local relative path when `path` is inside the project dir,
    /// absolute otherwise). Marks the project dirty + refreshes the panel
    /// context so the new entry shows immediately. No-op when the path is
    /// already registered; a project closed since the dialog opened is
    /// reported to the Messages panel.
    fn register_standalone_library_on_project(
        &mut self,
        project_id: crate::app::state::ProjectId,
        path: &std::path::Path,
    ) {
        use signex_types::project::LibraryEntry;
        let target_idx = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id);
        let Some(idx) = target_idx else {
            crate::diagnostics::log_warning(format!(
                "New library file {}: its project ({project_id}) was closed before the write \
                 finished. The file is already on disk but is not in any project; use Add \
                 Existing from inside the project to add it.",
                path.display()
            ));
            return;
        };
        let Some(loaded) = self.document_state.projects.get_mut(idx) else {
            return;
        };
        let project_dir = std::path::PathBuf::from(&loaded.data.dir);
        use crate::app::state::library_entry::{is_library_listed, library_entry_location};
        let fold = loaded.case_fold;
        let (entry_path, entry_kind) = library_entry_location(&project_dir, path, fold);
        // Skip if the same path is already on the list.
        if is_library_listed(&loaded.data, path, fold) {
            return;
        }
        loaded.data.libraries.push(LibraryEntry {
            path: entry_path,
            kind: entry_kind,
            library_id: None,
            unknown: Default::default(),
        });
        let project_path = loaded.path.clone();
        self.document_state.dirty_paths.insert(project_path);
        self.refresh_panel_ctx();
    }
}

/// Create `path`'s parent directory and write `text` atomically, refusing an existing `path`. Runs on a
/// blocking worker, never in `update`.
fn write_library_file(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {} failed: {e}", parent.display()))?;
    }
    // No-clobber: a new library must never replace an existing file.
    crate::app::handlers::dock::project_navigation::add_io::create_noclobber(path, |f| {
        std::io::Write::write_all(f, text.as_bytes())
    })
    .map_err(|e| format!("write {} failed: {e}", path.display()))
}

#[cfg(test)]
mod write_library_file_tests {
    use super::write_library_file;

    #[test]
    fn creating_over_an_existing_library_is_refused_and_leaves_it_untouched() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("new").join("lib.snxsym");
        write_library_file(&p, "first").unwrap();
        let err = write_library_file(&p, "second").unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "first");
    }
}
