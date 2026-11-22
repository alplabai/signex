//! The outline checks of a board sketch (mockup row 179d), as Properties and the bake
//! preview list them. They are read from the editor's current solve each
//! time the panel context is built, so every path that solves (a canvas
//! edit, a Properties edit, undo, a solve landing from its task) shows them
//! live. The inside-corner check uses the router tool diameter the board
//! session copied from its project. An open profile offers Close gap on
//! both hosts (mockup row 190). Rules: `docs/PCB_SKETCH.md`, "Outline checks".

use signex_bake::board::{
    CheckState, Finding, NotChecked, OutlineCheck, OutlineChecks, outline_checks_with_router,
};
use signex_bake::open_ends::{OpenEnd, bakes_a_profile, open_ends};
use signex_i18n::{MessageId, Translatable, ids};
use signex_sketch::SketchData;
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::heal::{HEAL_TOLERANCE_MM, heals};
use signex_sketch::id::SketchEntityId;
use signex_types::sketch::SketchHost;

use super::FootprintEditorState;

impl FootprintEditorState {
    /// The outline checks of `sketch` at the last solve, when this editor
    /// edits a board sketch. `None` on a footprint, and for a sketch with
    /// no solve (it failed, or none ran yet).
    pub fn board_checks(&self, sketch: Option<&SketchData>) -> Option<OutlineChecks> {
        match (self.sketch_host(), sketch) {
            (SketchHost::Board, Some(sketch)) => outline_checks_with_router(
                sketch,
                self.last_solve.as_ref(),
                self.sketch_exit.router_tool_diameter,
            ),
            _ => None,
        }
    }

    /// The gap Close gap would join in `sketch` at the last solve: the
    /// first open profile end paired with another, on either host. `None`
    /// when every profile is closed or nothing is solved.
    pub fn gap_offer(&self, sketch: Option<&SketchData>) -> Option<GapOffer> {
        let solve = self.last_solve.as_ref()?;
        open_ends(sketch?, solve, bakes_a_profile)
            .iter()
            .find_map(GapOffer::of)
    }

    /// The first open profile end of `sketch` at the last solve, paired or
    /// lone. `None` when every profile is closed or nothing is solved.
    pub fn first_open_end(&self, sketch: Option<&SketchData>) -> Option<OpenEnd> {
        let solve = self.last_solve.as_ref()?;
        open_ends(sketch?, solve, bakes_a_profile)
            .into_iter()
            .next()
    }
}

/// How a line of the Outline checks list reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Passed,
    Failed,
    NotChecked,
}

/// One line of the Outline checks list: its text, the sketch entity a
/// click on it selects, and for an open outline the Close gap offer.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckLine {
    pub status: CheckStatus,
    pub text: Translatable,
    pub place: Option<SketchEntityId>,
    pub close_gap: Option<GapOffer>,
}

/// Two open profile ends Close gap can be asked to join (mockup row 190). Only a gap
/// within the heal tolerance closes; a wider one is offered no button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapOffer {
    /// The end kept.
    pub keep: SketchEntityId,
    /// The end joined into it.
    pub merge: SketchEntityId,
    pub gap_mm: f64,
}

impl GapOffer {
    /// The offer of a paired open end; `None` for a lone end.
    pub fn of(end: &OpenEnd) -> Option<Self> {
        let (keep, merge) = end.pair()?;
        Some(Self {
            keep,
            merge,
            gap_mm: end.gap_mm?,
        })
    }

    /// Whether the gap is within the heal tolerance, so Close gap is shown.
    pub fn closes(&self) -> bool {
        heals(self.gap_mm)
    }

    /// The line that says whether the gap closes, naming the gap and the
    /// tolerance.
    pub fn text(&self) -> Translatable {
        let id = if self.closes() {
            ids::SKETCH_BAKE_GAP_CLOSES
        } else {
            ids::SKETCH_BAKE_GAP_TOO_WIDE
        };
        Translatable::new(id)
            .arg("gap", mm(self.gap_mm, 2))
            .arg("tolerance", mm(HEAL_TOLERANCE_MM, 2))
    }
}

/// The lines of `checks`: one per check, a failing check one per finding.
/// No checks (the sketch is not solved): one line that says so.
pub fn check_lines(checks: Option<&OutlineChecks>) -> Vec<CheckLine> {
    let Some(checks) = checks else {
        return vec![line(
            CheckStatus::NotChecked,
            Translatable::new(ids::BOARD_SKETCH_CHECKS_UNSOLVED),
        )];
    };
    let mut lines = Vec::new();
    for check in OutlineCheck::ALL {
        match checks.state(check) {
            CheckState::Passed => lines.push(line(
                CheckStatus::Passed,
                Translatable::new(passed_id(check)),
            )),
            CheckState::Failed(findings) => lines.extend(findings.iter().map(failed_line)),
            CheckState::NotChecked(why) => lines.push(line(
                CheckStatus::NotChecked,
                Translatable::new(skipped_id(*why)).arg("check", Translatable::new(name_id(check))),
            )),
        }
    }
    lines
}

/// A line that names no place and offers nothing.
fn line(status: CheckStatus, text: Translatable) -> CheckLine {
    CheckLine {
        status,
        text,
        place: None,
        close_gap: None,
    }
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
        NotChecked::NoRouterTool => ids::BOARD_SKETCH_CHECK_SKIPPED_ROUTER,
    }
}

/// A millimetre value as the lines show it.
pub fn mm(value: f64, decimals: usize) -> String {
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
        Finding::TightCorner {
            at,
            radius_mm,
            tool_radius_mm,
            ..
        } => Translatable::new(ids::BOARD_SKETCH_CHECK_CORNER_TIGHT)
            .arg("radius", mm(*radius_mm, 2))
            .arg("tool", mm(*tool_radius_mm, 2))
            .arg("x", mm(at.x, 2))
            .arg("y", mm(at.y, 2)),
    };
    let close_gap = match finding {
        Finding::OpenEnd {
            ends,
            gap_mm: Some(gap_mm),
            ..
        } => match ends.as_slice() {
            [keep, merge] => Some(GapOffer {
                keep: *keep,
                merge: *merge,
                gap_mm: *gap_mm,
            }),
            _ => None,
        },
        _ => None,
    };
    CheckLine {
        status: CheckStatus::Failed,
        text,
        place: finding.place().first().copied(),
        close_gap,
    }
}
