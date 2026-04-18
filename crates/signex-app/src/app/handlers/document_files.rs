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
            "kicad_pro" | "snxprj" => self.open_project_file(path)?,
            "kicad_sch" | "snxsch" => self.open_schematic_file(path)?,
            "kicad_pcb" | "snxpcb" => self.open_pcb_file(path)?,
            _ => anyhow::bail!("unsupported file type: .{ext}"),
        }

        Ok(())
    }

    fn open_project_file(&mut self, path: PathBuf) -> Result<()> {
        let project = kicad_parser::parse_project(&path)
            .with_context(|| format!("parse project {}", path.display()))?;
        self.document_state.project_path = Some(path);
        self.document_state.project_data = Some(project);
        self.refresh_panel_ctx();
        Ok(())
    }

    fn open_schematic_file(&mut self, path: PathBuf) -> Result<()> {
        let sheet = kicad_parser::parse_schematic_file(&path)
            .with_context(|| format!("parse schematic {}", path.display()))?;
        self.document_state.project_path = Some(path.clone());
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let project_path = dir.join(format!("{stem}.kicad_pro"));
            if project_path.exists() {
                match kicad_parser::parse_project(&project_path)
                    .with_context(|| format!("parse companion project {}", project_path.display()))
                {
                    Ok(project) => self.document_state.project_data = Some(project),
                    Err(error) => {
                        crate::diagnostics::log_error("Failed to parse companion project", &error)
                    }
                }
            }
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Schematic".to_string());
        self.open_schematic_tab(path, title, sheet);
        Ok(())
    }

    fn open_pcb_file(&mut self, path: PathBuf) -> Result<()> {
        let board = kicad_parser::parse_pcb_file(&path)
            .with_context(|| format!("parse pcb {}", path.display()))?;
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "PCB".to_string());
        self.document_state.project_path = Some(path.clone());
        self.open_pcb_tab(path, title, board);
        Ok(())
    }

    fn save_active_document(&mut self) -> Result<()> {
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
