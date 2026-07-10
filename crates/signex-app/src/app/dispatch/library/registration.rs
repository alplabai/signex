//! Library lifecycle handlers — creating a library for a project or
//! at a path, the open-library picker, adding standalone symbol /
//! footprint files, and registering a standalone library onto the
//! active project.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
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
                None => Message::Noop,
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
        let Some(picker) = self.library.picker.as_mut() else {
            return Task::none();
        };
        match msg {
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
                if let Some((path, summary)) = picker.selected.clone() {
                    tracing::warn!(
                        target: "signex::library",
                        library = %path.display(),
                        internal_pn = %summary.internal_pn.as_str(),
                        "place flow shipped in Phase 2 — picker dismissed"
                    );
                }
                self.library.picker = None;
            }
        }
        Task::none()
    }

    /// F34 — Save-As dialog confirmed for a new symbol library file
    /// (`.snxsym`). The user picked the location + filename in the
    /// rfd `save_file()` dialog — that click IS the explicit save
    /// action, so we write the empty `SymbolFile` to disk
    /// immediately, register the path on the containing project's
    /// `data.libraries` list (so the tree shows it directly under
    /// Libraries), then open it as a clean primitive editor tab
    /// (dirty=false). Subsequent edits flow through the regular
    /// `Ctrl+S → save_primitive_tab_at` path.
    pub(crate) fn handle_add_library_symbol_file_picked(
        &mut self,
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
        let text = match file.to_toml_string() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "AddLibrarySymbolFilePicked: serialize failed"
                );
                return Task::none();
            }
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                target: "signex::library",
                parent = %parent.display(),
                error = %e,
                "AddLibrarySymbolFilePicked: create symbols dir failed"
            );
            return Task::none();
        }
        if let Err(e) = signex_types::atomic_io::atomic_write(&path, text.as_bytes()) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "AddLibrarySymbolFilePicked: write .snxsym failed"
            );
            return Task::none();
        }
        self.register_standalone_library_on_project(&path);
        self.handle_open_primitive(path)
    }

    /// F34 — Footprint counterpart to
    /// [`handle_add_library_symbol_file_picked`]. Writes an empty
    /// `FootprintFile` (TOML+TSV envelope), registers the file as a
    /// project library entry, opens the file as a clean primitive
    /// editor tab.
    pub(crate) fn handle_add_library_footprint_file_picked(
        &mut self,
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
        let text = match file.to_toml_string() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "AddLibraryFootprintFilePicked: serialize failed"
                );
                return Task::none();
            }
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                target: "signex::library",
                parent = %parent.display(),
                error = %e,
                "AddLibraryFootprintFilePicked: create footprints dir failed"
            );
            return Task::none();
        }
        if let Err(e) = signex_types::atomic_io::atomic_write(&path, text.as_bytes()) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "AddLibraryFootprintFilePicked: write .snxfpt failed"
            );
            return Task::none();
        }
        self.register_standalone_library_on_project(&path);
        self.handle_open_primitive(path)
    }

    /// Find the project containing `path` and push a `LibraryEntry`
    /// for it onto `data.libraries` (project-local relative path when
    /// `path` is inside the project dir, absolute otherwise). Marks
    /// the project dirty + refreshes the panel context so the new
    /// entry shows immediately. No-op when the path is already
    /// registered, or when no loaded project owns the file's parent.
    fn register_standalone_library_on_project(&mut self, path: &std::path::Path) {
        use signex_types::project::{LibraryEntry, LibraryEntryKind};
        // Resolve the target project index first so the mutable borrow
        // of `projects` is short-lived (the active-project fallback
        // chained on iter_mut tripped E0500).
        let target_idx = self
            .document_state
            .projects
            .iter()
            .position(|p| {
                let project_dir = std::path::PathBuf::from(&p.data.dir);
                !project_dir.as_os_str().is_empty() && path.starts_with(&project_dir)
            })
            .or_else(|| {
                self.document_state
                    .active_project
                    .and_then(|id| self.document_state.projects.iter().position(|p| p.id == id))
            });
        let Some(idx) = target_idx else {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                "register_standalone_library: no project to attach to"
            );
            return;
        };
        let Some(loaded) = self.document_state.projects.get_mut(idx) else {
            return;
        };
        let project_dir = std::path::PathBuf::from(&loaded.data.dir);
        let (entry_path, entry_kind) = if !project_dir.as_os_str().is_empty()
            && let Ok(rel) = path.strip_prefix(&project_dir)
        {
            (rel.to_path_buf(), LibraryEntryKind::ProjectLocal)
        } else {
            (path.to_path_buf(), LibraryEntryKind::Shared)
        };
        // Skip if the same path is already on the list.
        if loaded
            .data
            .libraries
            .iter()
            .any(|e| loaded.data.resolve_library_path(e) == path)
        {
            return;
        }
        loaded.data.libraries.push(LibraryEntry {
            path: entry_path,
            kind: entry_kind,
            library_id: None,
        });
        let project_path = loaded.path.clone();
        self.document_state.dirty_paths.insert(project_path);
        self.refresh_panel_ctx();
    }

}
