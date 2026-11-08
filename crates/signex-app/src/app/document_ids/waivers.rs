//! The project's ERC waivers follow a sheet whose document id changes
//! (#2172): a net-level waiver names its sheet by id, so a Save As or a
//! repaired duplicate would otherwise orphan it.

use std::path::Path;

use signex_types::project::ErcWaiver;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::state::ProjectId;

/// How the waivers of the old id reach the new one.
#[derive(Clone, Copy)]
pub(crate) enum Follow {
    /// Save As: the copy replaces the sheet in the project, so they move.
    Move,
    /// A repaired duplicate: another sheet still holds the old id, so both keep them.
    Copy,
}

impl Signex {
    /// The project that lists the sheet at `sheet`, to re-key its waivers.
    pub(crate) fn waiver_owner_of(&self, sheet: &Path) -> Option<ProjectId> {
        self.document_state.document_project(sheet).map(|p| p.id)
    }

    /// Make the waivers of `project` filed under the sheet id `old` hold for
    /// `new`. The project's file now differs from its stored rules, so it is
    /// marked modified when any waiver changed, and Messages says so.
    pub(crate) fn follow_waivers(
        &mut self,
        project: Option<ProjectId>,
        old: Uuid,
        new: Uuid,
        how: Follow,
    ) {
        let Some(project) = project.and_then(|id| self.document_state.project_by_id_mut(id)) else {
            return;
        };
        let Some(rules) = project.data.rules.as_mut() else {
            return;
        };
        let waivers: &mut Vec<ErcWaiver> = &mut rules.erc_waivers;
        let changed = match how {
            Follow::Move => signex_erc::waivers::violation::rekey_sheet(waivers, old, new),
            Follow::Copy => signex_erc::waivers::violation::copy_sheet_waivers(waivers, old, new),
        };
        if changed == 0 {
            return;
        }
        let project_path = project.path.clone();
        self.document_state.mark_document_dirty(&project_path);
        crate::diagnostics::log_info(format!(
            "{changed} ERC waiver(s) now hold for a sheet's new id; save the project to keep them"
        ));
    }
}
