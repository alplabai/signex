//! The outline checks of a board sketch (mockup 179d), as Properties and the bake
//! preview list them. They are read from the editor's current solve each
//! time the panel context is built, so every path that solves (a canvas
//! edit, a Properties edit, undo, a solve landing from its task) shows them
//! live. Rules: `docs/PCB_SKETCH.md`, "Outline checks".

use signex_bake::board::{
    CheckState, Finding, NotChecked, OutlineCheck, OutlineChecks, outline_checks_after,
};
use signex_i18n::{MessageId, Translatable, ids};
use signex_sketch::SketchData;
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::id::SketchEntityId;
use signex_types::sketch::SketchHost;

use super::FootprintEditorState;

impl FootprintEditorState {
    /// The outline checks of `sketch` at the last solve, when this editor
    /// edits a board sketch. `None` on a footprint, and for a sketch with
    /// no solve (it failed, or none ran yet).
    pub fn board_checks(&self, sketch: Option<&SketchData>) -> Option<OutlineChecks> {
        match (self.sketch_host(), sketch) {
            (SketchHost::Board, Some(sketch)) => {
                outline_checks_after(sketch, self.last_solve.as_ref())
            }
            _ => None,
        }
    }
}

/// How a line of the Outline checks list reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Passed,
    Failed,
    NotChecked,
}

/// One line of the Outline checks list: its text, and the sketch entity a
/// click on it selects.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckLine {
    pub status: CheckStatus,
    pub text: Translatable,
    pub place: Option<SketchEntityId>,
}

/// The lines of `checks`: one per check, a failing check one per finding.
/// No checks (the sketch is not solved): one line that says so.
pub fn check_lines(checks: Option<&OutlineChecks>) -> Vec<CheckLine> {
    let Some(checks) = checks else {
        return vec![CheckLine {
            status: CheckStatus::NotChecked,
            text: Translatable::new(ids::BOARD_SKETCH_CHECKS_UNSOLVED),
            place: None,
        }];
    };
    let mut lines = Vec::new();
    for check in OutlineCheck::ALL {
        match checks.state(check) {
            CheckState::Passed => lines.push(CheckLine {
                status: CheckStatus::Passed,
                text: Translatable::new(passed_id(check)),
                place: None,
            }),
            CheckState::Failed(findings) => lines.extend(findings.iter().map(failed_line)),
            CheckState::NotChecked(why) => lines.push(CheckLine {
                status: CheckStatus::NotChecked,
                text: Translatable::new(skipped_id(*why))
                    .arg("check", Translatable::new(name_id(check))),
                place: None,
            }),
        }
    }
    lines
}

fn passed_id(check: OutlineCheck) -> MessageId {
    match check {
        OutlineCheck::Closed => ids::BOARD_SKETCH_CHECK_CLOSED,
        OutlineCheck::NoCrossing => ids::BOARD_SKETCH_CHECK_NO_CROSSING,
        OutlineCheck::OpeningsInside => ids::BOARD_SKETCH_CHECK_OPENINGS_INSIDE,
        OutlineCheck::InsideCorners => ids::BOARD_SKETCH_CHECK_CORNERS,
    }
}

fn name_id(check: OutlineCheck) -> MessageId {
    match check {
        OutlineCheck::Closed => ids::BOARD_SKETCH_CHECK_NAME_CLOSED,
        OutlineCheck::NoCrossing => ids::BOARD_SKETCH_CHECK_NAME_CROSSING,
        OutlineCheck::OpeningsInside => ids::BOARD_SKETCH_CHECK_NAME_OPENINGS,
        OutlineCheck::InsideCorners => ids::BOARD_SKETCH_CHECK_NAME_CORNERS,
    }
}

fn skipped_id(why: NotChecked) -> MessageId {
    match why {
        NotChecked::NoOutline => ids::BOARD_SKETCH_CHECK_SKIPPED_NO_OUTLINE,
        NotChecked::NeedsClosedOutline => ids::BOARD_SKETCH_CHECK_SKIPPED_NOT_CLOSED,
        NotChecked::Unsolved => ids::BOARD_SKETCH_CHECK_SKIPPED_UNSOLVED,
        NotChecked::NeedsRouterDiameter => ids::BOARD_SKETCH_CHECK_SKIPPED_ROUTER,
    }
}

/// A millimetre value as the lines show it.
fn mm(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

fn failed_line(finding: &Finding) -> CheckLine {
    let text = match finding {
        Finding::OpenEnd {
            gap_mm: Some(gap),
            at: Some(at),
            ..
        } => Translatable::new(ids::BOARD_SKETCH_CHECK_OPEN_GAP)
            .arg("gap", mm(*gap, 3))
            .arg("x", mm(at.x, 2))
            .arg("y", mm(at.y, 2)),
        Finding::OpenEnd { at: Some(at), .. } => Translatable::new(ids::BOARD_SKETCH_CHECK_OPEN_AT)
            .arg("x", mm(at.x, 2))
            .arg("y", mm(at.y, 2)),
        Finding::OpenEnd { at: None, .. } => Translatable::new(ids::BOARD_SKETCH_CHECK_OPEN),
        Finding::Branching { .. } => Translatable::new(ids::BOARD_SKETCH_CHECK_BRANCHING),
        Finding::Untraceable { .. } => Translatable::new(ids::BOARD_SKETCH_CHECK_UNTRACEABLE),
        Finding::SeveralLoops { count, .. } => {
            Translatable::new(ids::BOARD_SKETCH_CHECK_LOOPS).arg("count", count.to_string())
        }
        Finding::Crossing { at, .. } => Translatable::new(ids::BOARD_SKETCH_CHECK_CROSSING)
            .arg("x", mm(at.x, 2))
            .arg("y", mm(at.y, 2)),
        Finding::OpeningOutside { kind, .. } => Translatable::new(match kind {
            BakedKind::Cutouts => ids::BOARD_SKETCH_CHECK_CUTOUT_OUTSIDE,
            BakedKind::Keepouts => ids::BOARD_SKETCH_CHECK_KEEPOUT_OUTSIDE,
            BakedKind::MountingHoles => ids::BOARD_SKETCH_CHECK_HOLE_OUTSIDE,
            _ => ids::BOARD_SKETCH_CHECK_OPENING_OUTSIDE,
        }),
    };
    CheckLine {
        status: CheckStatus::Failed,
        text,
        place: finding.place().first().copied(),
    }
}
