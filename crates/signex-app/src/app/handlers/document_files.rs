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

    pub(crate) fn handle_active_document_save_requested(&mut self) {
        if let Err(error) = self.save_active_document() {
            crate::diagnostics::log_error("Failed to save active document", &error);
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

    fn save_active_document(&mut self) -> Result<()> {
        // Standalone `.snxsym` / `.snxfpt` document tabs route Ctrl+S
        // through `save_primitive_tab_at` so JSON persistence happens
        // before the generic schematic-save handler runs (it would
        // no-op for these tabs but the diagnostic log line would be
        // misleading).
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            match &active_tab.kind {
                super::super::TabKind::SymbolEditor(path)
                | super::super::TabKind::FootprintEditor(path) => {
                    let path = path.clone();
                    self.save_primitive_tab_at(&path);
                    crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
                    return Ok(());
                }
                _ => {}
            }
        }
        if let Some(result) = self.with_active_schematic_session_mut(|session| session.save()) {
            result.context("save active schematic session")?;
            let path = self.active_tab_path().unwrap_or_default();
            crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
        }
        Ok(())
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
}
