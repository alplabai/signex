use std::path::PathBuf;

use super::super::*;

impl Signex {
    pub(crate) fn handle_file_opened(&mut self, path: Option<PathBuf>) {
        let Some(path) = path else {
            // User cancelled file dialog
            return;
        };

        // Reset transient overlays when opening a new file/project.
        self.editing_text = None;
        self.context_menu = None;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "kicad_pro" | "snxprj" => {
                // Parse project file — discovers all sheets
                match kicad_parser::parse_project(&path) {
                    Ok(proj) => {
                        self.project_path = Some(path.clone());
                        self.project_data = Some(proj.clone());
                        // Don't auto-load any schematic — user clicks in project tree to open
                        self.refresh_panel_ctx();
                    }
                    Err(e) => eprintln!("Failed to parse project: {e}"),
                }
            }
            "kicad_sch" | "snxsch" => {
                // Direct schematic open — also try to find the .kicad_pro
                match kicad_parser::parse_schematic_file(&path) {
                    Ok(sheet) => {
                        self.project_path = Some(path.clone());
                        // Try to find and parse the .kicad_pro in the same directory
                        if let Some(dir) = path.parent() {
                            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                            let pro_path = dir.join(format!("{stem}.kicad_pro"));
                            if pro_path.exists()
                                && let Ok(proj) = kicad_parser::parse_project(&pro_path)
                            {
                                self.project_data = Some(proj);
                            }
                        }
                        let title = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Schematic".to_string());
                        self.open_schematic_tab(path.clone(), title, sheet);
                    }
                    Err(e) => eprintln!("Failed to parse schematic: {e}"),
                }
            }
            "kicad_pcb" | "snxpcb" => match kicad_parser::parse_pcb_file(&path) {
                Ok(board) => {
                    self.project_path = Some(path.clone());
                    let title = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "PCB".to_string());
                    self.open_pcb_tab(path.clone(), title, board);
                }
                Err(e) => eprintln!("Failed to parse pcb: {e}"),
            },
            _ => {
                eprintln!("Unsupported file type: .{ext}");
            }
        }
    }

    pub(crate) fn handle_save_file(&mut self) {
        if let Some(result) = self.with_active_schematic_session_mut(|session| session.save()) {
            match result {
                Ok(_) => {
                    let path = self.active_tab_path().unwrap_or_default();
                    #[cfg(debug_assertions)]
                    eprintln!("[save] Wrote {}", path.display());
                }
                Err(e) => {
                    eprintln!("[save] Error: {e}");
                    // TODO: show error in status bar / modal
                }
            }
        }
    }

    pub(crate) fn handle_save_file_as(&mut self, path: PathBuf) {
        if let Some(result) = self.with_active_schematic_session_mut(|session| session.save_as(path.clone())) {
            match result {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[save-as] Wrote {}", path.display());
                }
                Err(e) => {
                    eprintln!("[save-as] Error: {e}");
                    // TODO: show error in status bar / modal
                }
            }
        }
    }
}
