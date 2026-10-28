//! Project Options -> Designator standard (#1232): the write path.
//!
//! Mutates the open project's `ProjectData` and flips its `.snxprj` dirty; the
//! file is written by the normal explicit save (atomic write).

use signex_types::designator::DesignatorStandard;

use super::super::*;

impl Signex {
    /// Set (or, with `None`, clear) the designator standard of the project the
    /// Project Options modal is showing. Not an undo step: the picker is the
    /// whole edit surface and re-picking restores the previous value.
    pub(crate) fn handle_designator_standard_picked(
        &mut self,
        standard: Option<DesignatorStandard>,
    ) {
        let Some(idx) = self
            .ui_state
            .project_options
            .as_ref()
            .map(|o| o.project_idx)
        else {
            return;
        };
        let ds = &mut self.document_state;
        let Some(project) = ds.projects.get_mut(idx) else {
            return;
        };
        // An unreadable section from a newer Signex is replaced only by a real
        // standard; re-picking "Library designator" must not discard it.
        let unreadable = project.data.has_unreadable_annotation();
        if unreadable && standard.is_none() {
            return;
        }
        if !unreadable && project.data.designator_standard() == standard {
            return;
        }
        project.data.set_designator_standard(standard);
        let path = project.path.clone();
        ds.dirty_paths.insert(path);
        self.refresh_panel_ctx();
    }
}
