//! Per-channel variant state in the Variant Manager (#2662): a part on a
//! repeated sheet has one matrix row that opens into a row per channel's
//! copy, and each copy is set on its own like any part. The right pane of a
//! copy also sets every channel's copy at once, as one undo step of the
//! General history. Opening a row is view state only.

use signex_engine::ProjectSectionCommand;
use signex_types::pcb::SymbolLink;
use signex_types::project::PartRef;

use crate::app::Signex;
use crate::app::state::ProjectId;

impl Signex {
    /// Open the row of `part` (the part every channel's copy shares) into
    /// its copies, or close it.
    pub(super) fn toggle_channel_part(&mut self, project_id: ProjectId, part: SymbolLink) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            let open = &mut tab.variant_manager.open_channel_parts;
            if !open.remove(&part) {
                open.insert(part);
            }
        }
    }

    /// Fit or leave out every channel's copy of `part` in `variant`, or let
    /// the sheets decide for each again (`None`): one step, however many
    /// channels.
    pub(super) fn pick_channel_part_fit(
        &mut self,
        project_id: ProjectId,
        variant: String,
        part: &SymbolLink,
        fitted: Option<bool>,
    ) {
        let Some(matrix) = self.variant_read(project_id) else {
            return;
        };
        let parts: Vec<PartRef> = matrix
            .channel_copies(part)
            .iter()
            .map(|row| PartRef::new(row.designator.clone(), row.link.clone()))
            .collect();
        if parts.is_empty() {
            return;
        }
        let command = ProjectSectionCommand::SetVariantPartsFit {
            variant,
            parts,
            fitted,
        };
        self.apply_variant_manager_edit(project_id, &command);
    }
}
