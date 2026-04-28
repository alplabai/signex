use std::path::PathBuf;

use anyhow::{Context, Result};

use super::super::*;

impl Signex {
    pub(crate) fn handle_document_file_opened(&mut self, path: Option<PathBuf>) {
        let Some(path) = path else {
            return;
        };

        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;

        if let Err(error) = self.open_document_path(path) {
            crate::diagnostics::log_error("Failed to open document path", &error);
        }
    }

    pub(crate) fn handle_active_document_save_requested(&mut self) -> iced::Task<Message> {
        match self.save_active_document() {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Failed to save active document", &error);
                iced::Task::none()
            }
        }
    }

    pub(crate) fn handle_active_document_save_as_requested(&mut self, path: PathBuf) {
        if let Err(error) = self.save_active_document_as(path) {
            crate::diagnostics::log_error("Failed to save active document as", &error);
        }
    }

    fn open_document_path(&mut self, path: PathBuf) -> Result<()> {
        let ext = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("");
        match ext {
            "standard_pro" | "snxprj" => self.open_project_file(path)?,
            "standard_sch" | "snxsch" => self.open_schematic_file(path)?,
            "standard_pcb" | "snxpcb" => self.open_pcb_file(path)?,
            "snxsym" | "snxfpt" => {
                let _ = self.handle_open_primitive(path);
            }
            // `.snxlib/` is a directory package — open it as a Library
            // Browser tab in the main canvas area. The handler mounts
            // the library if not already mounted and pushes the tab
            // (or activates it if a tab for the same library is
            // already open).
            "snxlib" => {
                let _ = self.handle_open_library_browser(path);
            }
            _ => anyhow::bail!("unsupported file type: .{ext}"),
        }

        Ok(())
    }

    fn open_project_file(&mut self, path: PathBuf) -> Result<()> {
        self.load_or_activate_project(&path)?;
        self.refresh_panel_ctx();
        Ok(())
    }

    /// Append a `LoadedProject` for `project_path` to the workspace if
    /// it isn't already loaded, then make it active. De-dupes by path
    /// so re-opening the same project just switches activity. Used by
    /// both `open_project_file` (direct .standard_pro open) and the
    /// companion-project path inside `open_schematic_file` /
    /// `open_pcb_file`. Returns the resolved `ProjectId`.
    fn load_or_activate_project(
        &mut self,
        project_path: &std::path::Path,
    ) -> Result<crate::app::state::ProjectId> {
        if let Some(existing) = self
            .document_state
            .projects
            .iter()
            .find(|p| p.path == project_path)
        {
            let id = existing.id;
            self.document_state.active_project = Some(id);
            return Ok(id);
        }
        let data = standard_parser::parse_project(project_path)
            .with_context(|| format!("parse project {}", project_path.display()))?;
        let id = self.document_state.mint_project_id();
        // Auto-mount every library referenced by `Project::libraries`
        // so the project tree picks them up before the panel rebuild
        // fires. Errors are logged inside `auto_mount_project_libraries`
        // and never bubble: a missing library shouldn't block the
        // project from loading.
        let mounted =
            crate::library::commands::auto_mount_project_libraries(&mut self.library, &data);
        if mounted > 0 {
            tracing::info!(
                target: "signex::library",
                project = %project_path.display(),
                mounted,
                "auto-mounted project libraries"
            );
        }
        self.document_state
            .projects
            .push(super::super::state::LoadedProject {
                id,
                path: project_path.to_path_buf(),
                data,
            });
        self.document_state.active_project = Some(id);
        Ok(id)
    }

    fn open_schematic_file(&mut self, path: PathBuf) -> Result<()> {
        // Try to load the companion project so the schematic tab gets
        // a `project_id` via `project_for_path`. Best-effort: a missing
        // or unparseable `.standard_pro` doesn't block opening the loose
        // schematic.
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.standard_pro"));
            if companion.exists()
                && let Err(error) = self.load_or_activate_project(&companion)
            {
                crate::diagnostics::log_error("Failed to parse companion project", &error);
            }
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Schematic".to_string());
        // Parked-engine restore — same Altium-parity rule as the
        // project-tree open path. Reparsing from disk would discard
        // edits the user made before closing the tab.
        if self.document_state.engines.contains_key(&path)
            && self.document_state.dirty_paths.contains(&path)
        {
            self.attach_parked_schematic_tab(path, title);
            return Ok(());
        }
        let sheet = standard_parser::parse_schematic_file(&path)
            .with_context(|| format!("parse schematic {}", path.display()))?;
        self.open_schematic_tab(path, title, sheet);
        Ok(())
    }

    fn open_pcb_file(&mut self, path: PathBuf) -> Result<()> {
        let board = standard_parser::parse_pcb_file(&path)
            .with_context(|| format!("parse pcb {}", path.display()))?;
        // Same companion-project resolution as `open_schematic_file` so
        // the PCB tab can resolve `project_id` for project-scoped
        // handlers.
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.standard_pro"));
            if companion.exists()
                && let Err(error) = self.load_or_activate_project(&companion)
            {
                crate::diagnostics::log_error("Failed to parse companion project", &error);
            }
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "PCB".to_string());
        self.open_pcb_tab(path, title, board);
        Ok(())
    }

    fn save_active_document(&mut self) -> Result<iced::Task<Message>> {
        // Standalone `.snxsym` / `.snxfpt` document tabs route Ctrl+S
        // through `save_primitive_tab_at` so JSON persistence happens
        // before the generic schematic-save handler runs (it would
        // no-op for these tabs but the diagnostic log line would be
        // misleading). When the file doesn't exist on disk yet (the
        // tab was minted in-memory by `Add New ▸ Symbol` /
        // `Add New ▸ Footprint`), return a Task that opens an
        // AsyncFileDialog so the user can pick where the new
        // primitive lands — including a global library directory
        // outside the active project. The dialog result dispatches
        // `Message::SavePrimitiveAs { from, to }` which re-keys the
        // editor + tab and writes the file.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            match &active_tab.kind {
                super::super::TabKind::SymbolEditor(path)
                | super::super::TabKind::FootprintEditor(path) => {
                    let path = path.clone();
                    if !path.exists() {
                        return Ok(spawn_save_as_for_new_primitive(path));
                    }
                    self.save_primitive_tab_at(&path);
                    crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
                    return Ok(iced::Task::none());
                }
                _ => {}
            }
        }
        if let Some(result) = self.with_active_schematic_session_mut(|session| session.save()) {
            result.context("save active schematic session")?;
            let path = self.active_tab_path().unwrap_or_default();
            crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
        }
        Ok(iced::Task::none())
    }

    fn save_active_document_as(&mut self, path: PathBuf) -> Result<()> {
        if let Some(result) =
            self.with_active_schematic_session_mut(|session| session.save_as(path.clone()))
        {
            result.with_context(|| format!("save active schematic as {}", path.display()))?;
            crate::diagnostics::log_info(format!("[save-as] Wrote {}", path.display()));
        }
        Ok(())
    }

    /// Resolve `Message::SavePrimitiveAs { from_path, to_path }` —
    /// re-key the editor and tab from the in-memory `from_path` to
    /// the user-chosen `to_path`, then write the file via
    /// `save_primitive_tab_at`. Same machinery the in-memory editor
    /// already uses for atomic writes; just runs it under the new
    /// path the user picked.
    pub(crate) fn handle_save_primitive_as(
        &mut self,
        from_path: &std::path::Path,
        to_path: &std::path::Path,
    ) {
        // Reject re-keying onto a path that already hosts a different
        // open editor — would clobber the other editor's state.
        if from_path != to_path
            && (self.document_state.symbol_editors.contains_key(to_path)
                || self.document_state.footprint_editors.contains_key(to_path))
        {
            tracing::warn!(
                target: "signex::library",
                from = %from_path.display(),
                to = %to_path.display(),
                "save-as: target path already hosts another open editor — refusing"
            );
            return;
        }

        // Move the SymbolEditorState (or FootprintEditorState) to the
        // new key. The editor's internal `path` field is updated to
        // match so commit-through-adapter / refresh-cache callers see
        // the right path.
        if let Some(mut editor) = self.document_state.symbol_editors.remove(from_path) {
            editor.path = to_path.to_path_buf();
            self.document_state
                .symbol_editors
                .insert(to_path.to_path_buf(), editor);
        } else if let Some(mut editor) = self.document_state.footprint_editors.remove(from_path) {
            editor.path = to_path.to_path_buf();
            self.document_state
                .footprint_editors
                .insert(to_path.to_path_buf(), editor);
        } else {
            tracing::warn!(
                target: "signex::library",
                from = %from_path.display(),
                "save-as: no editor at source path — was the tab closed?"
            );
            return;
        }

        // Update the tab(s) — title, path, kind variant.
        let new_title = to_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| to_path.display().to_string());
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == from_path {
                tab.path = to_path.to_path_buf();
                tab.title = new_title.clone();
                tab.kind = match tab.kind {
                    super::super::TabKind::SymbolEditor(_) => {
                        super::super::TabKind::SymbolEditor(to_path.to_path_buf())
                    }
                    super::super::TabKind::FootprintEditor(_) => {
                        super::super::TabKind::FootprintEditor(to_path.to_path_buf())
                    }
                    _ => tab.kind.clone(),
                };
            }
        }

        // Migrate the dirty marker too — `dirty_paths` is keyed on
        // path so it has to follow the rename.
        if self.document_state.dirty_paths.remove(from_path) {
            self.document_state.dirty_paths.insert(to_path.to_path_buf());
        }

        // Now write the file at the new path. `atomic_write` inside
        // `save_primitive_tab_at` handles `create_dir_all(parent)` so
        // saving into a fresh `<lib>/symbols/` directory just works.
        self.save_primitive_tab_at(to_path);
        crate::diagnostics::log_info(format!("[save-as] Wrote {}", to_path.display()));

        // Library-tracking step. If the user saved into a `.snxlib`
        // directory that isn't mounted yet, mount it so the project
        // tree picks it up; if it isn't yet a member of the active
        // project's `libraries`, append it (project-local for paths
        // inside the project dir, Shared for paths outside) and
        // mark the project file dirty. The `.snxprj` itself isn't
        // re-serialised here — that's a separate persistence flow —
        // but the dirty marker means the next project-close prompt
        // will surface the unsaved library binding.
        self.attach_library_for_path(to_path);
    }

    /// Walk `path`'s ancestors looking for a `.snxlib` directory and,
    /// if found, make sure the active project has a `LibraryEntry`
    /// pointing at it. Mounts the library on the `LibrarySet` if it's
    /// not mounted yet. Logs and skips when there's no `.snxlib`
    /// ancestor (loose-file save outside the library system) or when
    /// no project is active to attach to.
    fn attach_library_for_path(&mut self, path: &std::path::Path) {
        let lib_dir = path.ancestors().find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("snxlib"))
                .unwrap_or(false)
        });
        let Some(lib_dir) = lib_dir else {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                "save-as: file is outside any `.snxlib/` — not attached to a library"
            );
            return;
        };

        // Mount through `state.open_library` (idempotent — re-opening a
        // mounted library is a no-op). Bail on failure: an invalid
        // library directory shouldn't poison the project's library list.
        if self.library.library_at(lib_dir).is_none() {
            if let Err(e) = crate::library::commands::open_library(
                &mut self.library,
                lib_dir.to_path_buf(),
            ) {
                tracing::warn!(
                    target: "signex::library",
                    path = %lib_dir.display(),
                    error = %e,
                    "save-as: open_library failed — leaving project untouched"
                );
                return;
            }
        }
        let library_id = self.library.library_at(lib_dir).map(|l| l.library_id);

        // Pick the project to attach to. Prefer the active project (so
        // the user's current focus is what gets the new entry); fall
        // back to the project that already contains `path` (rare —
        // would only happen if the user opened a primitive without an
        // active project, which the rest of the flow doesn't really
        // support). Skip silently when there's no project at all.
        let project_idx = match self.document_state.active_project {
            Some(active_id) => self
                .document_state
                .projects
                .iter()
                .position(|p| p.id == active_id),
            None => None,
        };
        let Some(project_idx) = project_idx else {
            tracing::warn!(
                target: "signex::library",
                path = %lib_dir.display(),
                "save-as: no active project to attach the library to"
            );
            return;
        };

        let loaded = &mut self.document_state.projects[project_idx];
        let project_dir = std::path::PathBuf::from(&loaded.data.dir);

        // Already in the project? Match by library_id when we have one
        // (handles renames); otherwise compare resolved paths.
        let already_attached = loaded.data.libraries.iter().any(|entry| {
            if let (Some(eid), Some(lid)) = (entry.library_id, library_id) {
                eid == lid
            } else {
                loaded.data.resolve_library_path(entry) == lib_dir
            }
        });
        if already_attached {
            self.refresh_panel_ctx();
            return;
        }

        // Project-local when the library lives inside the project dir,
        // Shared otherwise. Project-local stores the path relative to
        // the project so a project move doesn't break the binding.
        let (kind, stored_path) = if let Ok(rel) = lib_dir.strip_prefix(&project_dir) {
            (
                signex_types::project::LibraryEntryKind::ProjectLocal,
                rel.to_path_buf(),
            )
        } else {
            (
                signex_types::project::LibraryEntryKind::Shared,
                lib_dir.to_path_buf(),
            )
        };

        loaded.data.libraries.push(signex_types::project::LibraryEntry {
            path: stored_path,
            kind,
            library_id,
        });

        // Mark the `.snxprj` dirty so the project tree's red dot lights
        // up and the project-close prompt asks the user to persist the
        // new binding. (The actual `.snxprj` write isn't wired yet — a
        // follow-up commit will plumb that through Ctrl+S; for now the
        // in-session view is correct, persistence is best-effort.)
        let project_path = loaded.path.clone();
        self.document_state.dirty_paths.insert(project_path);

        tracing::info!(
            target: "signex::library",
            project = %loaded.path.display(),
            library = %lib_dir.display(),
            "save-as: attached library to active project"
        );

        self.refresh_panel_ctx();
    }
}

/// Build the AsyncFileDialog Task for a primitive's first save.
/// The dialog defaults to the suggested path's parent + filename so
/// the common case is a single Enter key; the user can navigate to
/// a global library directory outside the project if they want a
/// shared symbol. Cancel = no save (editor stays dirty).
pub(crate) fn spawn_save_as_for_new_primitive(suggested: PathBuf) -> iced::Task<Message> {
    let ext = suggested
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("snxsym")
        .to_string();
    let (filter_label, filter_ext) = match ext.as_str() {
        "snxfpt" => ("Signex Footprint", "snxfpt"),
        _ => ("Signex Symbol", "snxsym"),
    };
    let title = match ext.as_str() {
        "snxfpt" => "Save Footprint As",
        _ => "Save Symbol As",
    };
    let default_dir = suggested
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let default_name = suggested
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("New.{filter_ext}"));
    let from = suggested;

    iced::Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title(title)
                .add_filter(filter_label, &[filter_ext])
                .set_directory(&default_dir)
                .set_file_name(&default_name)
                .save_file()
                .await
                .map(|file| file.path().to_path_buf())
        },
        move |picked| match picked {
            Some(to_path) => Message::SavePrimitiveAs {
                from_path: from.clone(),
                to_path,
            },
            None => Message::Noop,
        },
    )
}
