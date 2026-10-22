//! The one resolver from a project-tree library row to its `.snxlib` path.

use super::*;

impl Signex {
    /// On-disk path of the library at `tree_path`
    /// (`[project, Libraries branch, library]`).
    ///
    /// The path comes from the project's live library list (saved entries,
    /// then pending ones), the order the tree rows are built in. It is
    /// refused (`None`) unless the tree's cached row still agrees with that
    /// list, so a stale index can never redirect an action to another
    /// library. Pure: callers that act on `None` report it themselves.
    pub(crate) fn library_row_path(&self, tree_path: &[usize]) -> Option<std::path::PathBuf> {
        let [project_idx, _, library_idx] = *tree_path else {
            return None;
        };
        let tree = self.document_state.panel_ctx.project_tree.as_slice();
        let branch = signex_widgets::tree_view::get_node(tree, &tree_path[..2])?;
        if branch.label != crate::panels::LIBRARIES_BRANCH_LABEL {
            return None;
        }
        let row = signex_widgets::tree_view::get_node(tree, tree_path)?;

        let project = self.document_state.projects.get(project_idx)?;
        let resolved = project
            .data
            .libraries
            .iter()
            .map(|entry| project.data.resolve_library_path(entry))
            .chain(
                project
                    .pending_libraries
                    .values()
                    .map(|s| s.lib_path.clone()),
            )
            .nth(library_idx)?;

        let cached = &self
            .document_state
            .panel_ctx
            .projects
            .get(project_idx)?
            .libraries
            .get(library_idx)?
            .root;
        let file_name = resolved.file_name().and_then(|n| n.to_str())?;
        let label_matches = row.label.trim_end_matches("  (missing)") == file_name;
        (*cached == resolved && label_matches).then_some(resolved)
    }
}
