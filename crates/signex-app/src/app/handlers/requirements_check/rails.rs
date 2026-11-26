//! The run behind `query_rail_budget` (#2943): every rail of the project
//! with its voltage, source, loads, total and headroom, from the project's
//! rails and the parts the BOM buys (`signex_req::rail_budget_answer`).
//! What can be is captured from memory in `update`; closed sheets and a
//! closed board are read off the UI thread, within the headless read limits
//! in a headless run or for an agent, as `check_requirements` reads them.
//! It changes nothing. See `docs/CLI.md`, "Rail budgets".

use serde_json::{Value, json};

use super::{board_path, bom, read_project_board};
use crate::app::Signex;
use crate::app::state::LoadedProject;

impl Signex {
    /// The rail budget answer of `project`, ready to run off the UI thread.
    /// Refused when the project's rails cannot be read or its BOM cannot be
    /// built.
    pub(crate) fn rail_budget_job(
        &self,
        project: &LoadedProject,
    ) -> impl FnOnce() -> Result<Value, String> + Send + 'static {
        let rails = project.data.rails();
        let confined = self.headless_answers().is_some() || self.agent.serving;
        let confine = confined.then(|| project.dir().to_path_buf());
        let board_path = board_path(project);
        let open_board = board_path
            .as_deref()
            .and_then(|path| self.document_state.pcb_board(path).cloned());
        let inputs = self.requirements_bom_inputs(project);
        let project_file = project.path.display().to_string();
        move || {
            let rails =
                rails.map_err(|why| format!("the project's rails cannot be read: {why}"))?;
            let board =
                board_path.map(|path| read_project_board(path, open_board, confine.as_deref()));
            let mut warnings = Vec::new();
            if let Some((path, Err(why))) = &board {
                warnings.push(format!(
                    "The board {path} could not be read ({why}), so parts it leaves out of the BOM are counted"
                ));
            }
            let read = board.as_ref().and_then(|(_, board)| board.as_ref().ok());
            let boms = bom::project_boms(inputs, read, confine.as_deref())?;
            warnings.extend(boms.warnings);
            let mut answer = signex_req::rail_budget_answer(&rails, &boms.parts);
            if let Value::Object(map) = &mut answer {
                map.insert("project".into(), json!(project_file));
                map.insert("warnings".into(), json!(warnings));
            }
            Ok(answer)
        }
    }
}
