//! The project's grouped BOM for `query_bom` (#2938), built off the UI
//! thread as `bom_cost` and the BOM export build it (`bom::project_boms`):
//! every sheet of the project, the library's current part names, the
//! variant asked for, and the parts the project's board leaves out.

use signex_output::bom::BomTable;

use super::{board_path, bom, read_project_board};
use crate::app::Signex;
use crate::app::state::LoadedProject;

/// The grouped BOM and what could not be read on the way.
pub(crate) struct ProjectBom {
    pub(crate) table: BomTable,
    pub(crate) warnings: Vec<String>,
}

impl Signex {
    /// The BOM of `project` for `variant` (`None`: the base design), ready
    /// to run off the UI thread. Captured here from memory: no file is read.
    pub(crate) fn project_bom_job(
        &self,
        project: &LoadedProject,
        variant: Option<String>,
    ) -> impl FnOnce() -> Result<ProjectBom, String> + Send + 'static {
        let confined = self.headless_answers().is_some() || self.agent.serving;
        let confine = confined.then(|| project.dir().to_path_buf());
        let board_path = board_path(project);
        let open_board = board_path
            .as_deref()
            .and_then(|path| self.document_state.pcb_board(path).cloned());
        let inputs = self.requirements_bom_inputs(project).for_variant(variant);
        move || {
            let board =
                board_path.map(|path| read_project_board(path, open_board, confine.as_deref()));
            let read = board.as_ref().and_then(|(_, board)| board.as_ref().ok());
            let boms = bom::project_boms(inputs, read, confine.as_deref())?;
            let mut warnings = boms.warnings;
            if let Some((path, Err(why))) = &board {
                warnings.push(format!(
                    "BOM: the board {path} could not be read ({why}), so parts its footprints leave out are listed"
                ));
            }
            Ok(ProjectBom {
                table: boms.grouped,
                warnings,
            })
        }
    }
}
