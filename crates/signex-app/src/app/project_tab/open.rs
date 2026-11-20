//! Open or focus a project's Project tab.

use crate::app::{Signex, TabInfo, TabKind};

impl Signex {
    /// Project Options (project tree): focus the project's tab, or open it.
    /// One tab per project.
    pub(crate) fn open_project_tab(&mut self, project_idx: usize) {
        let Some(project) = self.document_state.projects.get(project_idx) else {
            crate::diagnostics::log_warning(format!(
                "Project Options: no project at tree index {project_idx}"
            ));
            return;
        };
        let id = project.id;
        let path = project.path.clone();
        let existing = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.kind.as_project() == Some(id));
        let idx = match existing {
            Some(idx) => idx,
            None => {
                let kind = TabKind::Project(id);
                // A page tab: "<project> — Project" (docs/PROJECT_TAB.md,
                // "Page tabs").
                let title = self.page_tab_title(&kind).unwrap_or_default();
                self.document_state.project_tabs.entry(id).or_default();
                self.document_state.tabs.push(TabInfo {
                    title,
                    path,
                    cached_document: None,
                    project_id: Some(id),
                    kind,
                });
                self.document_state.tabs.len() - 1
            }
        };
        if idx != self.document_state.active_tab || existing.is_none() {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
    }
}
