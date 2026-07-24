//! Document/project open + create handlers. Split from `handlers/document_files.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::super::super::*;

impl Signex {
    pub(crate) fn handle_document_file_opened(
        &mut self,
        path: Option<PathBuf>,
    ) -> iced::Task<Message> {
        let Some(path) = path else {
            return iced::Task::none();
        };

        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;

        match self.open_document_path(path) {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Failed to open document path", &error);
                iced::Task::none()
            }
        }
    }

    pub(crate) fn handle_new_project_file(&mut self, path: Option<PathBuf>) -> iced::Task<Message> {
        let Some(path) = path else {
            return iced::Task::none();
        };

        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;

        match self.create_new_project(&path) {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Failed to create new project", &error);
                iced::Task::none()
            }
        }
    }

    /// Create a brand-new `.snxprj` at `path` plus a blank companion
    /// `<stem>.snxsch` in the same directory, then load the project and
    /// open the schematic as a tab. The `.snxprj` is written empty —
    /// `parse_project` is directory-driven and ignores file content.
    fn create_new_project(
        &mut self,
        project_path: &std::path::Path,
    ) -> Result<iced::Task<Message>> {
        if project_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("snxprj"))
            != Some(true)
        {
            anyhow::bail!(
                "new project path must end in .snxprj (got {})",
                project_path.display()
            );
        }
        let dir = project_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("project path has no parent directory"))?;
        let stem = project_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("project path has no file stem"))?
            .to_string();

        std::fs::create_dir_all(dir)
            .with_context(|| format!("create project directory {}", dir.display()))?;

        // Refuse to create a new project over an existing, non-empty
        // `.snxprj` — writing the empty marker below would truncate it
        // and destroy the user's project. A zero-byte file is a legacy
        // marker and safe to (re)write.
        if let Ok(meta) = std::fs::metadata(project_path)
            && meta.len() > 0
        {
            anyhow::bail!(
                "a project already exists at {} — open it instead of creating a new one over it",
                project_path.display()
            );
        }

        std::fs::write(project_path, b"")
            .with_context(|| format!("write project file {}", project_path.display()))?;

        let sch_path = dir.join(format!("{stem}.snxsch"));
        if !sch_path.exists() {
            let sheet = super::blank_schematic_sheet();
            let serialised = signex_types::format::SnxSchematic::new(sheet)
                .write_string()
                .context("serialise blank schematic")?;
            signex_types::atomic_io::atomic_write(&sch_path, serialised.as_bytes())
                .with_context(|| format!("write blank schematic {}", sch_path.display()))?;
        }

        // The project itself opens synchronously (a directory-driven
        // `.snxprj` parse, no heavy IO) — only the schematic tab's
        // read+parse below is async, via `open_document_path`'s
        // `snxsch` branch.
        let _project_task = self.open_document_path(project_path.to_path_buf())?;
        let schematic_task = if sch_path.exists() {
            // Best-effort: surface the schematic tab so the new project
            // lands the user on a drawable canvas. Errors here just
            // leave them on the Welcome view.
            match self.open_document_path(sch_path) {
                Ok(task) => task,
                Err(error) => {
                    crate::diagnostics::log_error("Open new project schematic", &error);
                    iced::Task::none()
                }
            }
        } else {
            iced::Task::none()
        };
        self.refresh_panel_ctx();
        Ok(schematic_task)
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

    fn open_document_path(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        let ext = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("");
        let task = match ext {
            "standard_pro" | "snxprj" => {
                self.open_project_file(path)?;
                iced::Task::none()
            }
            "standard_sch" | "snxsch" => self.open_schematic_file(path)?,
            "standard_pcb" | "snxpcb" => self.open_pcb_file(path)?,
            "snxsym" | "snxfpt" => {
                let _ = self.handle_open_primitive(path);
                iced::Task::none()
            }
            // `.snxlib/` is a directory package — open it as a Library
            // Browser tab in the main canvas area. The handler mounts
            // the library if not already mounted and pushes the tab
            // (or activates it if a tab for the same library is
            // already open).
            "snxlib" => {
                let _ = self.handle_open_library_browser(path);
                iced::Task::none()
            }
            _ => anyhow::bail!("unsupported file type: .{ext}"),
        };

        Ok(task)
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
        let data = signex_types::project::parse_project(project_path)
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
            .push(super::super::super::state::LoadedProject {
                id,
                path: project_path.to_path_buf(),
                data,
                pending_libraries: std::collections::HashMap::new(),
            });
        self.document_state.active_project = Some(id);
        Ok(id)
    }

    /// #478 review — dedup guard shared by `open_schematic_file` /
    /// `open_pcb_file`. Mirrors the activate-existing convention in
    /// `handle_open_primitive`: if `path` already has a tab, activate
    /// it. If it doesn't, but an async open for it is already in
    /// flight (spawned by a previous call, not yet completed), do
    /// nothing and let that completion create the tab — spawning a
    /// second `Task::perform` for the same path would let both
    /// completions push a tab aliasing one `document_state.engines`
    /// entry, and closing one would orphan the other. Returns `true`
    /// when the caller must stop and not spawn a new open `Task`.
    fn activate_or_skip_duplicate_open(&mut self, path: &std::path::Path) -> bool {
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return true;
        }
        self.document_state.pending_opens.contains(path)
    }

    fn open_schematic_file(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        if self.activate_or_skip_duplicate_open(&path) {
            return Ok(iced::Task::none());
        }

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
            return Ok(iced::Task::none());
        }
        // Read + parse off the UI thread — `update()` must never block
        // on disk IO (MVU rule 3). Mirrors `refresh_history_panel`'s
        // `Task::perform` + `spawn_blocking` pattern: the completion
        // message (`FileMsg::SchematicOpenFinished`) carries the
        // original path/title so the dispatch handler opens the tab
        // exactly as this function used to do inline.
        //
        // Mark the path in flight right before spawning — cleared in
        // both arms of `FileMsg::SchematicOpenFinished` (#478 review).
        self.document_state.pending_opens.insert(path.clone());
        let read_path = path.clone();
        Ok(iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_and_parse_schematic(&read_path))
                    .await
                    .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
            },
            move |result| {
                Message::File(FileMsg::SchematicOpenFinished {
                    path,
                    title,
                    result: result.map(Box::new),
                })
            },
        ))
    }

    fn open_pcb_file(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        if self.activate_or_skip_duplicate_open(&path) {
            return Ok(iced::Task::none());
        }

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
        // Read + parse off the UI thread — same reasoning as
        // `open_schematic_file` above. Mark the path in flight right
        // before spawning — cleared in both arms of
        // `FileMsg::PcbOpenFinished` (#478 review).
        self.document_state.pending_opens.insert(path.clone());
        let read_path = path.clone();
        Ok(iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_and_parse_pcb(&read_path))
                    .await
                    .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
            },
            move |result| {
                Message::File(FileMsg::PcbOpenFinished {
                    path,
                    title,
                    result: result.map(Box::new),
                })
            },
        ))
    }
}

/// Read + parse a `.snxsch` off the UI thread (the `spawn_blocking` body
/// for `open_schematic_file`). Stringifies the full `anyhow` context
/// chain so the error survives the `Task::perform` boundary (`Message`
/// is `Clone`; `anyhow::Error` is not) — same shape as `HistoryLoaded`'s
/// `Result<_, String>`.
fn read_and_parse_schematic(
    path: &std::path::Path,
) -> Result<signex_types::schematic::SchematicSheet, String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("read schematic {}", path.display()))
        .and_then(|text| {
            signex_types::format::SnxSchematic::parse(&text)
                .with_context(|| format!("parse schematic {}", path.display()))
                .map(|schematic| schematic.sheet)
        })
        .map_err(|err| format!("{err:#}"))
}

/// Read + parse a `.snxpcb` off the UI thread — see
/// `read_and_parse_schematic`.
fn read_and_parse_pcb(path: &std::path::Path) -> Result<signex_types::pcb::PcbBoard, String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("read pcb {}", path.display()))
        .and_then(|text| {
            signex_types::format::SnxPcb::parse(&text)
                .with_context(|| format!("parse pcb {}", path.display()))
                .map(|pcb| pcb.board)
        })
        .map_err(|err| format!("{err:#}"))
}
