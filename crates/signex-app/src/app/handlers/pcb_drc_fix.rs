//! Fix DRC findings on a named board (#2940): `fix_drc_violation` one
//! finding by its waiver key, `fix_drc` every finding of one rule kind. The
//! fixes are `signex_engine::pcb::drc_fix`; this runs them with Run DRC's
//! rules for the board (`board_design_rules`), lands them as one undo step
//! and answers with what was fixed, what was left and why, and what a check
//! of the board finds afterwards. The answer's shape: `docs/CLI.md`,
//! "Fixing DRC findings".

use std::path::Path;

use serde_json::{Value, json};
use signex_drc::{DrcReport, Violation, WaiverState};
use signex_engine::pcb::drc_fix::{FixRun, FixScope, Fixed, Left, fix_findings};
use signex_i18n::{ids, t};
use signex_types::object_id::UuidV7Ids;
use signex_types::pcb::PcbBoard;

use crate::app::Signex;
use crate::app::state::pcb_edit::CheckedStep;

impl Signex {
    /// Fix what `scope` names on the open board `board`; with `dry_run`,
    /// answer with the fixes and change nothing. A fix that touches a
    /// locked item or makes a new finding is not made; no finding is waived.
    pub(crate) fn fix_drc(
        &mut self,
        board: &Path,
        scope: &FixScope,
        dry_run: bool,
    ) -> Result<Value, String> {
        let not_open = || format!("{} is not open", board.display());
        let pcb = self
            .document_state
            .pcb_board(board)
            .cloned()
            .ok_or_else(not_open)?;
        let (rules, _) = self.board_design_rules(board, &pcb)?;
        let engine = self
            .document_state
            .board_engine(board)
            .ok_or_else(not_open)?;
        let expected_revision = engine.revision();
        let run = fix_findings(engine, &rules, scope, &mut UuidV7Ids).map_err(|e| e.to_string())?;
        let changes = !dry_run && !run.commands.is_empty();
        let mut answer = run_json(board, &run, dry_run, changes);
        if !changes {
            // A dry run says what would remain; nothing to land says what is.
            answer["remaining"] = remaining_json(&run.report, &run.board);
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
        let now = self.document_state.pcb_board(board).ok_or_else(not_open)?;
        answer["remaining"] = remaining_json(&signex_drc::run(now, &rules), now);
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
