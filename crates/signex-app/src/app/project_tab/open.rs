//! Open or focus a project's Project tab.

use crate::app::{Signex, TabInfo, TabKind};

/// The Project tab's title: the `.snxprj` file name, so it never reads
/// like the root sheet's tab, which shows the same stem.
pub(crate) fn project_tab_title(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

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
                self.document_state.project_tabs.entry(id).or_default();
                self.document_state.tabs.push(TabInfo {
                    title: project_tab_title(&path),
                    path,
                    cached_document: None,
                    project_id: Some(id),
                    kind: TabKind::Project(id),
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
