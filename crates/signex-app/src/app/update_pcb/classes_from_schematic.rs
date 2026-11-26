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

    /// How many net classes Execute adds or changes for the open dialog;
    /// zero before the list is read.
    pub(crate) fn update_pcb_class_count(&self, state: &super::UpdatePcbState) -> usize {
        let (Some(snapshot), Some(board)) = (state.snapshot.as_deref(), state.board.as_deref())
        else {
            return 0;
        };
        self.update_pcb_class_edit(board, snapshot)
            .map_or(0, |edit| classes_in(&edit))
    }
}

/// How many classes `edit` adds or changes: the count the dialog's summary
/// line and its Execute count show.
pub(crate) fn classes_in(edit: &NetClassCommand) -> usize {
    fn walk<'c>(command: &'c NetClassCommand, names: &mut Vec<&'c str>) {
        let name = match command {
            NetClassCommand::Batch { commands } => {
                for command in commands {
                    walk(command, names);
                }
                return;
            }
            NetClassCommand::Add { class } => class.name.as_str(),
            NetClassCommand::AssignNet {
                class: Some(name), ..
            }
            | NetClassCommand::SetBoardRules { name, .. } => name.as_str(),
            _ => return,
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let mut names: Vec<&str> = Vec::new();
    walk(edit, &mut names);
    names.len()
}
