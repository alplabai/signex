//! The project a board belongs to: the one Run DRC and `check_drc` read
//! net classes and rules from (#2332).

use std::path::Path;

use super::scope::path_key_with;
use super::{DocumentState, LoadedProject};
use crate::app::document_ids::{DocumentKind, referenced_files};

impl DocumentState {
    /// The project of `board`: the one its document entry names (`named`),
    /// else the open project whose `pcb_file` is the board; `None` for a
    /// loose board. The entry of an open board names no project (ownership
    /// follows listed sheets only), while a closed one's does.
    pub(crate) fn board_project(
        &self,
        board: &Path,
        named: Option<&Path>,
    ) -> Option<&LoadedProject> {
        if let Some(named) = named {
            return self.projects.iter().find(|project| project.path == named);
        }
        // Two open projects listing the same `pcb_file`: the first loaded wins.
        self.projects.iter().find(|project| {
            let key = |path: &Path| path_key_with(path, project.case_fold);
            referenced_files(project)
                .iter()
                .any(|(path, kind)| *kind == DocumentKind::Pcb && key(path) == key(board))
        })
    }
}
