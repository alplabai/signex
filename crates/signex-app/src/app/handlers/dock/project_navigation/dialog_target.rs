//! The project a Remove or Rename dialog was opened for, resolved by id at
//! confirm time rather than by its position in the project list (#855).

use super::*;

impl Signex {
    /// The id of the project that owns the tree row at `tree_path`.
    pub(super) fn project_id_at_tree_path(
        &self,
        tree_path: &[usize],
    ) -> Option<crate::app::state::ProjectId> {
        let idx = *tree_path.first()?;
        self.document_state.projects.get(idx).map(|p| p.id)
    }

    /// Re-point a dialog's `tree_path[0]` at the current index of the project
    /// it was opened for, since projects may have been added or removed
    /// meanwhile (#855). Returns `false` when that project is no longer open.
    /// `None` (no captured project) leaves `tree_path` unchanged.
    pub(super) fn repoint_dialog_tree_path(
        &self,
        project_id: Option<crate::app::state::ProjectId>,
        tree_path: &mut [usize],
    ) -> bool {
        let Some(id) = project_id else {
            return true;
        };
        let Some(idx) = self.document_state.projects.iter().position(|p| p.id == id) else {
            return false;
        };
        if let Some(first) = tree_path.first_mut() {
            *first = idx;
        }
        true
    }
}
