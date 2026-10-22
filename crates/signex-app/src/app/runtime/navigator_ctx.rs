//! Keeps the Navigator panel's sheet hierarchy current.
//!
//! `refresh_panel_ctx` runs after nearly every update, and assembling the
//! project's sheets reads the closed ones from disk. So the hierarchy is only
//! rebuilt when what it depends on — the project's pages and root, the open
//! tabs, and the child-sheet references of the open sheets — has changed.
//! Everything cheaper (active-row highlight, open markers) is re-derived each
//! time from the cached hierarchy.
//!
//! Known limit: a *closed* child sheet that changes on disk (renamed, deleted,
//! corrupted) is not noticed until a tab opens or closes, because stat-ing
//! every sheet on each refresh would cost more than the staleness does.

use std::path::PathBuf;

use super::super::*;
use crate::app::project_sheets::{assemble_project_sheets, project_root_sheet_path};
use crate::app::sheet_hierarchy::build_sheet_hierarchy;
use crate::panels::NavigatorModel;

impl Signex {
    /// The Navigator model for the current state, reusing `previous`'s cached
    /// hierarchy when nothing it was built from has changed.
    pub(super) fn next_navigator_model(&self, mut previous: NavigatorModel) -> NavigatorModel {
        let Some((root, pages)) = self.navigator_project_scope() else {
            return NavigatorModel::default();
        };

        let open: Vec<PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        let active = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());

        let fingerprint = self.navigator_fingerprint(&root, &pages);
        if previous.fingerprint != fingerprint || previous.hierarchy.is_none() {
            let set = assemble_project_sheets(&self.document_state, &pages, &root);
            // Said once per rebuild, not per refresh: the reason a sheet that
            // exists could not be read would otherwise only be a bare marker.
            for (path, why) in &set.unreadable {
                crate::diagnostics::log_warning(format!(
                    "Navigator: sheet {} is unreadable: {why}",
                    path.display()
                ));
            }
            let hierarchy = build_sheet_hierarchy(
                &set.sheets,
                &set.unreadable,
                &root,
                &set.pages_outside_the_hierarchy,
            );
            previous.replace_hierarchy(hierarchy, active.as_deref(), &open);
            previous.fingerprint = fingerprint;
        } else {
            previous.rebuild_tree(active.as_deref(), &open);
        }
        previous
    }

    /// The project the Navigator describes, as `(root sheet, declared pages)`.
    ///
    /// The project owning the focused document, else the workspace's active
    /// project. Ownership alone is not enough: it is discovered through the
    /// *open* sheets' child references, so a leaf opened while its ancestors
    /// are closed resolves to no project — and the Navigator would go empty
    /// exactly when the user has navigated deep.
    pub(crate) fn navigator_project_scope(&self) -> Option<(PathBuf, Vec<PathBuf>)> {
        let project = self
            .document_state
            .active_document_project()
            .or_else(|| self.document_state.active_loaded_project())?;
        let root = project_root_sheet_path(project)?;
        let dir = project.dir();
        let pages = project
            .data
            .sheets
            .iter()
            .map(|s| dir.join(&s.filename))
            .collect();
        Some((root, pages))
    }

    fn navigator_fingerprint(&self, root: &std::path::Path, pages: &[PathBuf]) -> String {
        let mut lines = vec![format!("root {}", root.display())];
        lines.extend(pages.iter().map(|p| format!("page {}", p.display())));
        lines.extend(
            self.document_state
                .tabs
                .iter()
                .map(|t| format!("tab {}", t.path.display())),
        );
        let mut engines: Vec<_> = self.document_state.engines.iter().collect();
        engines.sort_by(|a, b| a.0.cmp(b.0));
        for (path, engine) in engines {
            lines.push(format!("sheet {}", path.display()));
            lines.extend(
                engine
                    .document()
                    .child_sheets
                    .iter()
                    .map(|c| format!("  ref {}|{}", c.name, c.filename)),
            );
        }
        lines.join("\n")
    }
}
