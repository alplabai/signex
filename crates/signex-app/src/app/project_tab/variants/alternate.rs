//! Alternate… in the Variant Manager's right pane (mockup rows 32, 208):
//! the canvas's Alternate part picker opens for the selected cell's part in
//! the cell's variant. A channel's copy opens with that channel shown and
//! the scope offered; any other part opens on its first placed unit.

use signex_types::project::PartRef;

use super::VariantCell;
use crate::app::state::ProjectId;
use crate::app::variant_part_edit::ManagedPart;
use crate::app::{Message, Signex};

impl Signex {
    /// Open the picker for `cell` of the project's parts read; nothing for
    /// Base's cell or a part the read no longer lists.
    pub(super) fn open_alternate_of_cell(
        &mut self,
        project_id: ProjectId,
        cell: &VariantCell,
    ) -> iced::Task<Message> {
        let Some(part) = self.managed_part(project_id, cell) else {
            return iced::Task::none();
        };
        self.open_alternate_for(part)
    }

    /// The cell's part as the picker takes it.
    fn managed_part(&self, project_id: ProjectId, cell: &VariantCell) -> Option<ManagedPart> {
        let variant = cell.variant.clone()?;
        let matrix = self.variant_read(project_id)?;
        let row = matrix
            .rows
            .iter()
            .find(|row| row.designator == cell.designator)?;
        let this_part = PartRef::new(row.designator.clone(), row.link.clone());
        let Some(channel) = row.channel.as_ref() else {
            return Some(ManagedPart {
                project: project_id,
                variant,
                this_part,
                channel: None,
                channels: None,
                every_channel: Vec::new(),
            });
        };
        let copies = matrix.channel_copies(&channel.part);
        let name = |row: Option<&&signex_output::VariantRow>| {
            row.and_then(|row| row.channel.as_ref())
                .map(|channel| channel.name.clone())
        };
        let channels = name(copies.first()).zip(name(copies.last()));
        Some(ManagedPart {
            project: project_id,
            variant,
            this_part,
            channel: Some(channel.name.clone()),
            channels,
            every_channel: copies
                .iter()
                .map(|copy| PartRef::new(copy.designator.clone(), copy.link.clone()))
                .collect(),
        })
    }
}
