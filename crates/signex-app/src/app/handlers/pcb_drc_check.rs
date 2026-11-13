//! The DRC run behind `check_drc` (#2199, decisions
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6817821301 and
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6818011610):
//! Run DRC's checks and rule-layer choice (`pcb_drc::run_check`,
//! `drc_layers`) over one board, with the net classes and rules of the
//! board's own project (else this machine's layer and a loose-board
//! warning; Run DRC uses the active project, #2332). An open board is read
//! from memory, in the GUI process and in headless runs; over the agent
//! channel a board open in the app is refused until #2331. A closed board is
//! read off the UI thread. It changes no DRC state and says nothing in
//! Messages; what it found comes back as a `signex_types::check` report. See
//! `docs/CLI.md`, "Checks on demand".

use std::path::Path;

use serde_json::{Map, Value, json};
use signex_types::check::CheckReport;
use signex_types::pcb::PcbBoard;
use signex_types::project::ProjectRules;

use super::pcb_drc::{Layers, drc_layers, run_check};
use crate::app::Signex;
use crate::app::state::LoadedProject;

/// The id a report names its check by.
pub(crate) const DRC_CHECK: &str = signex_drc::findings::DRC_CHECK;

/// What a report of a board in no project says.
pub(crate) const LOOSE_BOARD_WARNING: &str = "The board is in no open project: it was checked with its own setup rules and this machine's DRC rules, without net classes";

impl Signex {
    /// The check of `board`, in `project` (`None`: a loose board), ready to
    /// run off the UI thread. Captured here from memory: no file is read. A
    /// closed board is read by the job; a headless run or an agent reads only
    /// what a headless query may (`document_ids::confined_read`).
    pub(crate) fn drc_check_job(
        &self,
        board: &Path,
        project: Option<&LoadedProject>,
    ) -> impl FnOnce() -> Result<CheckReport, String> + Send + 'static {
        let open = self.document_state.pcb_board(board).cloned();
        let classes = project
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default();
        let stored = project.and_then(|p| p.data.rules.as_ref());
        let (profile, project_layer) = drc_layers(stored, &self.ui_state.machine_drc_rules);
        let layers = Layers {
            profile: profile.cloned(),
            project: project_layer.cloned(),
        };
        let confined = self.headless_answers().is_some() || self.agent.serving;
        let confine = project.filter(|_| confined).map(|p| p.dir().to_path_buf());
        let rule_set = self.drc_rule_set(project, stored);
        let warnings: Vec<String> = project
            .is_none()
            .then(|| LOOSE_BOARD_WARNING.to_string())
            .into_iter()
            .collect();
        let path = board.to_path_buf();
        move || {
            let board = match open {
                Some(board) => board,
                None => read_board(&path, confine.as_deref())?,
            };
            let (report, names) = run_check(&board, &classes, &layers)?;
            let name = path.display().to_string();
            Ok(signex_drc::findings::check_report(
                &report, &names, &name, warnings, rule_set,
            ))
        }
    }

    /// Where the rule layers came from: the project's stored rules or this
    /// machine's, its profile (as the ERC report names it) and the hash of
    /// the `drc` section used. `stored` is `project`'s stored rule set.
    fn drc_rule_set(
        &self,
        project: Option<&LoadedProject>,
        stored: Option<&ProjectRules>,
    ) -> Map<String, Value> {
        let (source, section) = match stored {
            Some(rules) => ("project", &rules.drc),
            None => ("machine", &self.ui_state.machine_drc_rules),
        };
        let mut out = Map::new();
        out.insert("source".into(), json!(source));
        out.insert(
            "profile".into(),
            json!(self.rule_set_identity_of(project).profile_name),
        );
        out.insert("sha256".into(), json!(section.sha256_hex()));
        out
    }
}

/// The board file at `path`, read and parsed; refused when it lies outside
/// the project folder `confine` or is no file a headless run reads.
fn read_board(path: &Path, confine: Option<&Path>) -> Result<PcbBoard, String> {
    if let Some(dir) = confine {
        crate::app::document_ids::confined_read(dir, path)?;
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{} could not be read: {e}", path.display()))?;
    signex_types::format::SnxPcb::parse(&text)
        .map(|parsed| parsed.board)
        .map_err(|e| format!("{} could not be read: {e}", path.display()))
}
