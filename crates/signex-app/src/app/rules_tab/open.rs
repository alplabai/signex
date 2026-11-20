//! Open or focus a project's Rules tab. One tab per project; its path is
//! the `.snxprj` the rules are stored in, so Save writes them.

use crate::app::state::ProjectId;
use crate::app::{Signex, TabInfo, TabKind};

impl Signex {
    /// The project the Rules command opens: `project`, else the active
    /// document's, else the active one.
    pub(crate) fn rules_project_for(&self, project: Option<ProjectId>) -> Option<ProjectId> {
        let docs = &self.document_state;
        project
            .filter(|id| docs.project_by_id(*id).is_some())
            .or_else(|| {
                docs.tabs
                    .get(docs.active_tab)
                    .and_then(|tab| tab.project_id)
            })
            .or_else(|| docs.active_document_project().map(|p| p.id))
    }

    /// Focus `id`'s Rules tab, or open it.
    pub(crate) fn open_rules_tab(&mut self, id: ProjectId) {
        let Some(project) = self.document_state.project_by_id(id) else {
            return;
        };
        let path = project.path.clone();
        let existing = self
            .document_state
            .tabs
            .iter()
            .position(|tab| matches!(tab.kind, TabKind::Rules(held) if held == id));
        let idx = match existing {
            Some(idx) => idx,
            None => {
                let kind = TabKind::Rules(id);
                // A page tab: "<project> — Rules" (docs/PROJECT_TAB.md,
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
