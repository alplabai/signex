//! The run behind `check_requirements` (#2339): the project's requirements
//! evaluated by `signex_req` over facts prepared here, only those the
//! listed requirements need. What can be is captured from memory in
//! `update` (open sheets and boards, net classes, the library rows'
//! lifecycle states, the cached prices); the ERC and DRC runs (the same
//! jobs as `check_erc` and `check_drc`), closed sheets and a closed board are
//! read off the UI thread, within the headless read limits in a headless run
//! or for an agent. It changes nothing and says nothing in Messages. See
//! `docs/CLI.md`, "Requirement checks".

pub(super) mod bom;
mod bom_job;
mod rails;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_req::{Board, Facts, Needs};
use signex_types::check::CheckReport;
use signex_types::pcb::PcbBoard;

use super::pcb_drc_check::read_board;
use crate::app::Signex;
use crate::app::project_sheets::project_root_sheet_path;
use crate::app::state::LoadedProject;
use crate::library::state::LibraryState;

/// The id a report names its check by.
pub(crate) const REQUIREMENTS_CHECK: &str = signex_req::REQUIREMENTS_CHECK;

/// A check run the requirements use, ready to run off the UI thread.
type Run = Box<dyn FnOnce() -> Result<CheckReport, String> + Send>;

impl Signex {
    /// The check of `project`'s requirements, ready to run off the UI
    /// thread. Captured here from memory: no file is read.
    pub(crate) fn requirements_check_job(
        &self,
        project: &LoadedProject,
    ) -> impl FnOnce() -> Result<CheckReport, String> + Send + 'static {
        let job = self.requirements_measured_check_job(project);
        move || Ok(job().0)
    }

    /// The check of `project`'s requirements as `requirements_check_job`,
    /// with the board outline's width and height in mm as the run measured
    /// them (`None`: no board was read, or it has no outline), for the
    /// Requirements tab's Board size preview.
    pub(crate) fn requirements_measured_check_job(
        &self,
        project: &LoadedProject,
    ) -> impl FnOnce() -> (CheckReport, Option<(f64, f64)>) + Send + 'static {
        let data = &project.data;
        let needs = Needs::of(&data.requirements);
        let confined = self.headless_answers().is_some() || self.agent.serving;
        let confine = confined.then(|| project.dir().to_path_buf());
        let board_path = board_path(project);
        let erc: Option<Run> = needs.erc.then(|| self.requirements_erc_run(project));
        let drc: Option<Run> = match (&board_path, needs.drc) {
            (Some(path), true) => Some(Box::new(self.drc_check_job(path, Some(project)))),
            _ => None,
        };
        let wants_board = needs.board || needs.drc || needs.parts || needs.bom;
        let open_board = board_path
            .as_deref()
            .filter(|_| wants_board)
            .and_then(|path| self.document_state.pcb_board(path).cloned());
        let boms = (needs.parts || needs.bom).then(|| self.requirements_bom_inputs(project));
        let base = Facts {
            project: project.path.display().to_string(),
            requirements: data.requirements.clone(),
            unreadable_list: data.has_unreadable_requirements(),
            erc: None,
            drc: None,
            board: Board::NotListed,
            classes: data.net_classes.clone(),
            parts: None,
            row_states: if needs.parts {
                row_states(&self.library)
            } else {
                HashMap::new()
            },
            bom: None,
            prices: if needs.bom {
                crate::library::bom_prices::price_book(&self.library)
            } else {
                Default::default()
            },
            rails: data.rails(),
            warnings: Vec::new(),
        };
        move || {
            let mut facts = base;
            facts.erc = erc.map(|run| run());
            facts.drc = drc.map(|run| run());
            let board = board_path
                .filter(|_| wants_board)
                .map(|path| read_project_board(path, open_board, confine.as_deref()));
            if let Some(inputs) = boms {
                let read = board.as_ref().and_then(|(_, board)| board.as_ref().ok());
                match bom::project_boms(inputs, read, confine.as_deref()) {
                    Ok(boms) => {
                        facts.warnings.extend(boms.warnings);
                        facts.parts = needs.parts.then_some(Ok(boms.parts));
                        facts.bom = needs.bom.then_some(Ok(boms.grouped));
                    }
                    Err(why) => {
                        facts.parts = needs.parts.then(|| Err(why.clone()));
                        facts.bom = needs.bom.then_some(Err(why));
                    }
                }
            }
            facts.board = match board {
                None => Board::NotListed,
                Some((path, Ok(board))) => Board::Read {
                    path,
                    board: Box::new(board),
                },
                Some((path, Err(why))) => Board::Unreadable { path, why },
            };
            let report = signex_req::check_report(&facts);
            (report, signex_req::board_extent_mm(&facts.board))
        }
    }

    /// The ERC run of `project`, as `check_erc` runs it from the project's
    /// root sheet; refused when the project names none.
    fn requirements_erc_run(&self, project: &LoadedProject) -> Run {
        match project_root_sheet_path(project) {
            Some(root) => {
                let job = self.erc_check_job(&root, Some(project));
                Box::new(move || Ok(job()))
            }
            None => Box::new(|| Err("the project has no root sheet".to_string())),
        }
    }
}

/// The project's board file, when it lists one.
pub(crate) fn board_path(project: &LoadedProject) -> Option<PathBuf> {
    let file = project.data.pcb_file.as_deref().map(str::trim)?;
    (!file.is_empty()).then(|| project.dir().join(file))
}

/// The board at `path` as the report names it: open from memory, else read
/// (within `confine`, when given).
fn read_project_board(
    path: PathBuf,
    open: Option<PcbBoard>,
    confine: Option<&Path>,
) -> (String, Result<PcbBoard, String>) {
    let board = match open {
        Some(board) => Ok(board),
        None => read_board(&path, confine),
    };
    (path.display().to_string(), board)
}

/// The lifecycle state of every row of every open library, by its library
/// row key.
fn row_states(library: &LibraryState) -> HashMap<String, String> {
    let mut states = HashMap::new();
    for open in &library.open_libraries {
        let library_id = open.library_id.to_string();
        for row in open.tables.values().flatten() {
            let key = signex_output::bom::library_row_key(&library_id, &row.row_id.to_string());
            states.insert(key, row.state.as_str().to_string());
        }
    }
    states
}

#[cfg(test)]
mod lifecycle_tests;
