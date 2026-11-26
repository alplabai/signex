//! Fix DRC findings on a named board (#2940): `fix_drc_violation` one
//! finding by its waiver key, `fix_drc` every finding of one rule kind. The
//! fixes are `signex_engine::pcb::drc_fix`, worked out off the UI thread on
//! a copy of the board with Run DRC's rules (`board_design_rules`) and the
//! parts' 3D bodies (`pcb_drc_bodies`), as `check_drc` checks them. They
//! land as one undo step built on the revision they were worked out from,
//! so a board edited meanwhile refuses them. The answer's shape:
//! `docs/CLI.md`, "Fixing DRC findings".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_drc::{BodyModels, DrcReport, DrcRules, Violation, WaiverState};
use signex_engine::pcb::drc_fix::{FixRun, FixScope, Fixed, Left, fix_findings};
use signex_i18n::{Translatable, ids, t};
use signex_types::object_id::UuidV7Ids;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, PcbMsg, Signex};
use crate::keymap::AppCommandId;

/// A fix run worked out off the UI thread, waiting to land.
#[derive(Debug, Clone)]
pub struct DrcFixComputed {
    pub(crate) waiting: WaitingCommand,
    pub(crate) board: PathBuf,
    /// The board revision the fixes were worked out from.
    pub(crate) expected_revision: u64,
    pub(crate) dry_run: bool,
    pub(crate) result: Result<Arc<FixRun>, String>,
}

impl PartialEq for DrcFixComputed {
    fn eq(&self, other: &Self) -> bool {
        let same_result = match (&self.result, &other.result) {
            (Ok(a), Ok(b)) => Arc::ptr_eq(a, b),
            (Err(a), Err(b)) => a == b,
            _ => false,
        };
        self.waiting == other.waiting
            && self.board == other.board
            && self.expected_revision == other.expected_revision
            && self.dry_run == other.dry_run
            && same_result
    }
}

/// A fix run's refusal, the reason under the catalog's line.
pub(crate) fn fix_refused(why: impl Into<String>) -> CommandOutcome {
    let reason = Translatable::new(ids::DRC_FIX_REFUSED).arg("why", why.into());
    CommandOutcome::Disabled { reason }
}

/// The fixes of `scope` on `board`, with the models of its parts read
/// first when a rule measures a body. Blocking IO and many DRC passes: a
/// worker's job.
fn fixes_off_thread(
    board: &PcbBoard,
    rules: &DrcRules,
    libraries: &[(Uuid, PathBuf)],
    scope: &FixScope,
) -> Result<FixRun, String> {
    let models = if rules.measures_bodies() {
        super::pcb_drc_bodies::body_models(board, libraries).0
    } else {
        BodyModels::new()
    };
    fix_findings(board, rules, &models, scope, &mut UuidV7Ids).map_err(|e| e.to_string())
}

impl Signex {
    /// Start working out the fixes `scope` names on the open board `board`
    /// off the UI thread; the run is answered when they land. With
    /// `dry_run`, the answer says what would change and nothing lands. A fix
    /// that touches a locked item or makes a new finding is not made; no
    /// finding is waived.
    pub(crate) fn start_fix_drc(
        &mut self,
        id: &str,
        args: &CommandArgs,
        board: PathBuf,
        scope: FixScope,
        dry_run: bool,
    ) -> Result<(CommandOutcome, Task<Message>), String> {
        let not_open = || format!("{} is not open", board.display());
        let pcb = self
            .document_state
            .pcb_board(&board)
            .cloned()
            .ok_or_else(not_open)?;
        let (rules, _) = self.board_design_rules(&board, &pcb)?;
        let expected_revision = self
            .document_state
            .board_engine(&board)
            .ok_or_else(not_open)?
            .revision();
        let command = AppCommandId::new(id).map_err(|e| e.to_string())?;
        let libraries = self.open_library_roots();
        let waiting = self.wait_for_command(&command, args);
        let ticket = waiting.ticket;
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    fixes_off_thread(&pcb, &rules, &libraries, &scope)
                })
                .await
                .map_err(|e| format!("the fixes did not finish: {e}"))
                .and_then(|run| run)
            },
            move |result| {
                Message::Pcb(PcbMsg::DrcFixComputed(Box::new(DrcFixComputed {
                    waiting: waiting.clone(),
                    board: board.clone(),
                    expected_revision,
                    dry_run,
                    result: result.map(Arc::new),
                })))
            },
        );
        Ok((CommandOutcome::Deferred { ticket }, task))
    }

    /// The fixes a waiting run worked out landed: make them as one undo step
    /// on the revision they were worked out from, and answer.
    pub(crate) fn handle_drc_fix_computed(&mut self, done: DrcFixComputed) -> Task<Message> {
        let DrcFixComputed {
            waiting,
            board,
            expected_revision,
            dry_run,
            result,
        } = done;
        let target = board.clone();
        self.finish_waiting_command(waiting, &target, move |app| {
            match app.land_drc_fixes(&board, expected_revision, dry_run, result) {
                Ok(answer) => CommandOutcome::Answered(answer),
                Err(why) => fix_refused(why),
            }
        })
    }

    fn land_drc_fixes(
        &mut self,
        board: &Path,
        expected_revision: u64,
        dry_run: bool,
        result: Result<Arc<FixRun>, String>,
    ) -> Result<Value, String> {
        let run = Arc::unwrap_or_clone(result?);
        let changes = !dry_run && !run.commands.is_empty();
        let mut answer = run_json(board, &run, dry_run, changes);
        // The check of the board the fixes leave, bodies included: what a
        // dry run would leave, and what is there once they land.
        answer["remaining"] = remaining_json(&run.report, &run.board);
        if !changes {
            return Ok(answer);
        }
        let label = t!(self.ui_state.i18n, ids::DRC_FIX_UNDO).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision,
        };
        self.execute_board_commands(board, run.commands, Some(step))
            .map_err(|e| e.to_string())?;
        self.refresh_panel_ctx();
        Ok(answer)
    }
}

/// What a run fixed and left, before the check after it.
fn run_json(board: &Path, run: &FixRun, dry_run: bool, changed: bool) -> Value {
    json!({
        "board": board.display().to_string(),
        "dry_run": dry_run,
        "changed": changed,
        "fixed": run.fixed.iter().map(fixed_json).collect::<Vec<_>>(),
        "left": run.left.iter().map(left_json).collect::<Vec<_>>(),
    })
}

/// A finding as the answer names it: its waiver key, rule, words and items.
fn finding_json(key: &str, v: &Violation) -> Value {
    json!({
        "id": key,
        "rule": signex_drc::rule_id(v),
        "kind": v.kind.rule_id(),
        "message": signex_drc::findings::describe(v),
        "items": v.items.iter().map(signex_drc::findings::item_id).collect::<Vec<_>>(),
        "at_mm": [signex_drc::findings::mm(v.at.x), signex_drc::findings::mm(v.at.y)],
    })
}

fn fixed_json(fixed: &Fixed) -> Value {
    let mut out = finding_json(&fixed.key, &fixed.violation);
    out["changed"] = json!(fixed.changed);
    out["cleared_by"] = json!(fixed.cleared_by);
    out
}

fn left_json(left: &Left) -> Value {
    let mut out = finding_json(&left.key, &left.violation);
    out["reason"] = json!(left.reason.code());
    out["why"] = json!(left.reason.message());
    out
}

/// The open findings of `report` on `board`, a rule switched on and not
/// waived, and how many are waived.
fn remaining_json(report: &DrcReport, board: &PcbBoard) -> Value {
    let shown = report
        .violations
        .iter()
        .filter(|v| report.severity_of(v).is_some());
    let (mut open, mut waived) = (Vec::new(), 0);
    for v in shown {
        match signex_drc::waiver_state(&board.options.drc_waivers, v, board) {
            WaiverState::Waived(_) => waived += 1,
            WaiverState::Lapsed(_) | WaiverState::Open => {
                open.push(finding_json(&signex_drc::waiver_key(v), v));
            }
        }
    }
    json!({"count": open.len(), "waived": waived, "findings": open})
}

#[cfg(test)]
#[path = "pcb_drc_fix_tests.rs"]
mod tests;
