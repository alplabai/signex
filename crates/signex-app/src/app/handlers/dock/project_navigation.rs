use anyhow::{Context, Result};

use super::super::super::*;

impl Signex {
    pub(super) fn handle_dock_project_navigation_panel_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        use signex_widgets::tree_view::{TreeIcon, TreeMsg, get_node};

        match panel_msg {
            crate::panels::PanelMsg::Tree(TreeMsg::Toggle(path)) => {
                signex_widgets::tree_view::toggle(&mut self.document_state.panel_ctx.project_tree, path);
                true
            }
            crate::panels::PanelMsg::Tree(TreeMsg::Select(path)) => {
                let selected_node = get_node(self.document_state.panel_ctx.project_tree.as_slice(), path);
                if let Some(node) = selected_node
                    && matches!(node.icon, TreeIcon::Schematic | TreeIcon::Pcb)
                    && let Err(error) = self.open_project_tree_document(node.label.clone())
                {
                    crate::diagnostics::log_error("Failed to open project tree document", &error);
                }
                true
            }
            _ => false,
        }
    }

    fn open_project_tree_document(&mut self, filename: String) -> Result<()> {
        let project_dir = self
            .document_state
            .project_path
            .as_ref()
            .and_then(|path| path.parent())
            .with_context(|| format!("resolve project directory for {}", filename))?;
        let file_path = project_dir.join(&filename);
        if !file_path.exists() {
            anyhow::bail!("project tree file does not exist: {}", file_path.display());
        }

        if let Some(index) = self.document_state.tabs.iter().position(|tab| tab.path == file_path) {
            if index != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = index;
                self.sync_active_tab();
            }
            return Ok(());
        }

        if filename.ends_with(".kicad_sch") || filename.ends_with(".snxsch") {
            let schematic = kicad_parser::parse_schematic_file(&file_path)
                .with_context(|| format!("parse schematic {}", file_path.display()))?;
            self.open_schematic_tab(file_path, filename.replace(".kicad_sch", ""), schematic);
            return Ok(());
        }

        if filename.ends_with(".kicad_pcb") || filename.ends_with(".snxpcb") {
            let board = kicad_parser::parse_pcb_file(&file_path)
                .with_context(|| format!("parse pcb {}", file_path.display()))?;
            let title = filename
                .trim_end_matches(".kicad_pcb")
                .trim_end_matches(".snxpcb")
                .to_string();
            self.open_pcb_tab(file_path, title, board);
            return Ok(());
        }

        anyhow::bail!("unsupported project tree document: {filename}")
    }
}