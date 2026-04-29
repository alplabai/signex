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

    pub(crate) fn handle_active_document_save_requested(
        &mut self,
    ) -> iced::Task<crate::app::Message> {
        match self.snapshot_active_document_for_save() {
            Ok(Some((path, bytes))) => self.spawn_async_save(path, bytes),
            Ok(None) => iced::Task::none(),
            Err(error) => {
                crate::diagnostics::log_error("Failed to snapshot active document", &error);
                iced::Task::none()
            }
        }
    }

    pub(crate) fn handle_active_document_save_as_requested(
        &mut self,
        path: PathBuf,
    ) -> iced::Task<crate::app::Message> {
        match self.snapshot_active_document_for_save_as(path) {
            Ok(Some((path, bytes))) => self.spawn_async_save(path, bytes),
            Ok(None) => iced::Task::none(),
            Err(error) => {
                crate::diagnostics::log_error("Failed to snapshot active document", &error);
                iced::Task::none()
            }
        }
    }

    /// Completion arm of the async save (v0.9.1). Off-thread write
    /// finished — clear the "Saving…" pill, mark the engine + tab
    /// state as clean (or surface the error briefly).
    pub(crate) fn handle_active_document_save_finished(
        &mut self,
        path: PathBuf,
        result: Result<(), String>,
    ) {
        self.ui_state.saving_paths.remove(&path);
        match result {
            Ok(()) => {
                // Mark engine clean. The engine may live under a
                // different key than `path` if this was a "Save As"
                // — for the current implementation we only fire async
                // saves for the active tab so look up the engine via
                // active_path. If by the time the async write returns
                // the user closed the tab, just clear the dirty flag
                // entry and move on.
                let active_engine_path = self.document_state.active_path.clone();
                if let Some(active_path) = active_engine_path {
                    if active_path != path {
                        // Save-As: re-key the engine and the tab.
                        if let Some(mut engine) = self.document_state.engines.remove(&active_path) {
                            engine.record_saved_path(path.clone());
                            self.document_state.engines.insert(path.clone(), engine);
                            self.document_state.active_path = Some(path.clone());
                        }
                        // Update the tab whose path was active_path.
                        if let Some(tab) = self
                            .document_state
                            .tabs
                            .iter_mut()
                            .find(|tab| tab.path == active_path)
                        {
                            tab.path = path.clone();
                            tab.title = path
                                .file_stem()
                                .map(|stem| stem.to_string_lossy().to_string())
                                .unwrap_or_else(|| "Schematic".to_string());
                            tab.dirty = false;
                        }
                        self.document_state.dirty_paths.remove(&active_path);
                    } else if let Some(engine) = self.document_state.engines.get_mut(&path) {
                        engine.record_saved_path(path.clone());
                    }
                }
                // Plain save: just clear the dirty flag for `path`.
                if let Some(tab) = self
                    .document_state
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.path == path)
                {
                    tab.dirty = false;
                }
                self.document_state.dirty_paths.remove(&path);
                crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
                self.ui_state.save_error = None;
            }
            Err(message) => {
                crate::diagnostics::log_error(
                    "Async save failed",
                    &anyhow::Error::msg(message.clone()),
                );
                self.ui_state.save_error = Some((message, std::time::Instant::now()));
            }
        }
    }

    fn snapshot_active_document_for_save(&mut self) -> anyhow::Result<Option<(PathBuf, Vec<u8>)>> {
        let Some(engine) = self.document_state.active_engine() else {
            return Ok(None);
        };
        let Some(path) = self.active_tab_path() else {
            return Ok(None);
        };
        let bytes = engine
            .serialize_for_save()
            .map_err(|e| anyhow::Error::msg(e.to_string()))
            .context("snapshot active schematic")?;
        Ok(Some((path, bytes)))
    }

    fn snapshot_active_document_for_save_as(
        &mut self,
        path: PathBuf,
    ) -> anyhow::Result<Option<(PathBuf, Vec<u8>)>> {
        let Some(engine) = self.document_state.active_engine() else {
            return Ok(None);
        };
        let bytes = engine
            .serialize_for_save()
            .map_err(|e| anyhow::Error::msg(e.to_string()))
            .with_context(|| format!("snapshot active schematic as {}", path.display()))?;
        Ok(Some((path, bytes)))
    }

    /// Hand the bytes to a worker task. Serialise already happened on
    /// the UI thread (cheap borrow-based path); the task does only
    /// the disk write. iced's tokio runtime is multi-threaded so the
    /// blocking `std::fs::write` runs on a runtime worker, leaving
    /// the UI thread responsive — this is the v0.9.1 win.
    fn spawn_async_save(
        &mut self,
        path: PathBuf,
        bytes: Vec<u8>,
    ) -> iced::Task<crate::app::Message> {
        self.ui_state.saving_paths.insert(path.clone());
        let path_for_task = path.clone();
        iced::Task::perform(
            async move {
                signex_engine::Engine::write_to_file(&path_for_task, &bytes)
                    .map_err(|error| error.to_string())
            },
            move |result| crate::app::Message::SaveFileFinished(path.clone(), result),
        )
    }

    fn open_document_path(&mut self, path: PathBuf) -> Result<()> {
        let ext = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("");
        match ext {
            "snxprj" => self.open_project_file(path)?,
            "snxsch" => self.open_schematic_file(path)?,
            "snxpcb" => self.open_pcb_file(path)?,
            "snxlib" => self.open_library_file(path)?,
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
    /// both `open_project_file` (direct .snxprj open) and the
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
        let data = signex_types::project::parse_project(project_path)
            .with_context(|| format!("parse project {}", project_path.display()))?;
        let id = self.document_state.mint_project_id();
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
        // or unparseable `.snxprj` doesn't block opening the loose
        // schematic.
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.snxprj"));
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
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read schematic {}", path.display()))?;
        let sheet = signex_types::format::SnxSchematic::parse(&text)
            .with_context(|| format!("parse schematic {}", path.display()))?
            .sheet;
        self.open_schematic_tab(path, title, sheet);
        Ok(())
    }

    fn open_pcb_file(&mut self, path: PathBuf) -> Result<()> {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read pcb {}", path.display()))?;
        let board = signex_types::format::SnxPcb::parse(&text)
            .with_context(|| format!("parse pcb {}", path.display()))?
            .board;
        // Same companion-project resolution as `open_schematic_file` so
        // the PCB tab can resolve `project_id` for project-scoped
        // handlers.
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.snxprj"));
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

    /// v0.10.0 — open a `.snxlib` package as a Library Browser tab.
    /// Read-only: the table view in `view_center` renders the parsed
    /// library directly. No companion-project resolution because
    /// library packages aren't currently scoped to a project (a
    /// library lives anywhere on the filesystem and is mounted across
    /// projects in v0.10.8+).
    fn open_library_file(&mut self, path: PathBuf) -> Result<()> {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read library {}", path.display()))?;
        let library = signex_types::format::SnxLibrary::parse(&text)
            .with_context(|| format!("parse library {}", path.display()))?
            .library;
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Library".to_string());
        self.open_library_tab(path, title, library);
        Ok(())
    }
}
