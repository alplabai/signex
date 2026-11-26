//! Update PCB's half of the classes the schematic's Net Blanket and Net
//! Parameters directives give (exit criterion 3): Execute puts each class,
//! its nets and its rules into the board's net classes, the project's
//! net-class table, as one step of that table's history. Which classes and
//! how they merge is domain logic in
//! `signex_engine::net_class_table::schematic_class_edit`;
//! `docs/SCHEMATIC_DIRECTIVES.md`, "Update PCB classes".

use std::path::Path;

use signex_engine::{NetClassCommand, schematic_class_edit};

use super::DesignSnapshot;

impl crate::app::Signex {
    /// The edit Execute makes to `board`'s net classes for `snapshot`;
    /// `None` when they already say it all or the board is in no project.
    pub(crate) fn update_pcb_class_edit(
        &self,
        board: &Path,
        snapshot: &DesignSnapshot,
    ) -> Option<NetClassCommand> {
        let project = self.document_state.board_project(board, None)?;
        schematic_class_edit(
            &project.data.net_classes,
            &snapshot.schematic_classes,
            &project.data.schematic_design.diff_pair_classes,
        )
    }

    /// Execute's class half: that edit as one step of the table's history.
    /// Nothing to change writes nothing.
    pub(crate) fn apply_update_pcb_classes(
        &mut self,
        board: &Path,
        snapshot: &DesignSnapshot,
    ) -> Result<(), String> {
        match self.update_pcb_class_edit(board, snapshot) {
            Some(edit) => self.edit_board_net_classes(board, edit).map(|_| ()),
            None => Ok(()),
        }
    }
}
