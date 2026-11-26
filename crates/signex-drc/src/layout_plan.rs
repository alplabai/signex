//! How far each stage of a board's layout plan is (#2942): not started,
//! done or failing, with the reasons, from the board, its project's net
//! classes and requirement ids, and the board's last DRC run when it is
//! current. Pure: nothing here edits the board or runs a check. The rules
//! per stage are in `docs/CLI.md`, "Layout plan".

use std::collections::BTreeMap;

use pcb_geom::{GeomError, PointNm, Polygon, checked_body, net_reports};
use signex_types::net_class::NetClass;
use signex_types::pcb::{Footprint, PcbBoard, Room, Zone};
use signex_types::pcb_layout_plan::{LayoutPlan, LayoutStage, RouteTarget};

use crate::{DrcReport, ViolationKind, WaiverState, rooms, rule_id, waiver_state};

/// The most items a reason list names; the rest are counted.
pub const LISTED: usize = 20;

/// Where a stage stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageState {
    NotStarted,
    Done,
    Failing,
}

impl StageState {
    pub const fn token(self) -> &'static str {
        match self {
            StageState::NotStarted => "not_started",
            StageState::Done => "done",
            StageState::Failing => "failing",
        }
    }
}

/// One stage's state, why it is not done, and what it counted.
#[derive(Debug, Clone, PartialEq)]
pub struct StageStatus {
    pub stage: LayoutStage,
    pub state: StageState,
    /// One line per reason; empty when done.
    pub reasons: Vec<String>,
    /// Named counts: `parts`/`placed`, `nets`/`routed`, `pours`/`filled`,
    /// `findings`/`waived`.
    pub counts: BTreeMap<&'static str, usize>,
}

/// What the status reads besides the plan.
pub struct PlanInput<'a> {
    pub board: &'a PcbBoard,
    /// The net classes of the board's project.
    pub net_classes: &'a [NetClass],
    /// The ids of the project's requirements.
    pub requirements: &'a [String],
    /// The board's last DRC run, only when it ran on the board as it is now.
    pub drc: Option<&'a DrcReport>,
}

/// Whether `id` names a DRC rule: a built-in kind, or any written rule.
pub fn is_known_rule(id: &str) -> bool {
    ViolationKind::ALL.iter().any(|kind| kind.rule_id() == id)
        || id
            .strip_prefix("user::")
            .is_some_and(|name| !name.is_empty())
}

/// What the plan names that the board or its project does not have, one
/// line each, in stage order.
pub fn missing_references(plan: &LayoutPlan, input: &PlanInput) -> Vec<String> {
    LayoutStage::ALL
        .into_iter()
        .flat_map(|stage| missing_in(stage, plan, input))
        .collect()
}

fn missing_in(stage: LayoutStage, plan: &LayoutPlan, input: &PlanInput) -> Vec<String> {
    let board = input.board;
    let mut missing = Vec::new();
    match stage {
        LayoutStage::Placement => {
            for name in &plan.placement.rooms {
                if !board.rooms.iter().any(|room| &room.name == name) {
                    missing.push(format!("room `{name}` is not on the board"));
                }
            }
            for designator in &plan.placement.keep {
                if !board
                    .footprints
                    .iter()
                    .any(|fp| &fp.reference == designator)
                {
                    missing.push(format!("part `{designator}` is not on the board"));
                }
            }
        }
        LayoutStage::Routing => {
            for target in &plan.routing.nets {
                match target {
                    RouteTarget::Net(name) if pcb_geom::net_named(board, name).is_none() => {
                        missing.push(format!("net `{name}` is not on the board"));
                    }
                    RouteTarget::Class(name)
                        if !input.net_classes.iter().any(|c| &c.name == name) =>
                    {
                        missing.push(format!("net class `{name}` is not in the project"));
                    }
                    _ => {}
                }
            }
            for name in &plan.routing.skip {
                if pcb_geom::net_named(board, name).is_none() {
                    missing.push(format!("net `{name}` is not on the board"));
                }
            }
        }
        LayoutStage::Pours => {
            for id in &plan.pours.pours {
                if pour_of(board, *id).is_none() {
                    missing.push(format!("pour {id} is not on the board"));
                }
            }
        }
        LayoutStage::Checks => {
            for id in &plan.checks.requirements {
                if !input.requirements.contains(id) {
                    missing.push(format!("requirement `{id}` is not in the project"));
                }
            }
        }
    }
    missing
}

/// Each stage of `plan` on `input`, in stage order.
pub fn plan_status(plan: &LayoutPlan, input: &PlanInput) -> Vec<StageStatus> {
    LayoutStage::ALL
        .into_iter()
        .map(|stage| {
            let mut status = match stage {
                LayoutStage::Placement => placement(plan, input),
                LayoutStage::Routing => routing(plan, input),
                LayoutStage::Pours => pours(plan, input),
                LayoutStage::Checks => checks(plan, input),
            };
            let mut missing = missing_in(stage, plan, input);
            if !missing.is_empty() {
                // A stage that names what is not there cannot be done.
                status.state = StageState::Failing;
                missing.append(&mut status.reasons);
                status.reasons = capped(missing);
            }
            status
        })
        .collect()
}

/// `done` of `total` items, with the reasons for the ones not done; `begun`
/// says whether any work of the stage is on the board yet.
fn judged(
    stage: LayoutStage,
    counts: [(&'static str, usize); 2],
    begun: bool,
    mut open: Vec<String>,
    none: &str,
) -> StageStatus {
    let [(_, total), (_, done)] = counts;
    let state = if total == 0 {
        open = vec![none.to_string()];
        StageState::NotStarted
    } else if done == total {
        StageState::Done
    } else if !begun {
        StageState::NotStarted
    } else {
        StageState::Failing
    };
    StageStatus {
        stage,
        state,
        reasons: if state == StageState::Done {
            Vec::new()
        } else {
            capped(open)
        },
        counts: counts.into_iter().collect(),
    }
}

/// `lines` with at most [`LISTED`] named and the rest counted.
fn capped(mut lines: Vec<String>) -> Vec<String> {
    if lines.len() > LISTED {
        let more = lines.len() - LISTED;
        lines.truncate(LISTED);
        lines.push(format!("and {more} more"));
    }
    lines
}

fn ring(
    points: &[signex_types::pcb::Point],
    field: &'static str,
) -> Result<Vec<PointNm>, GeomError> {
    points.iter().map(|p| PointNm::from_mm(*p, field)).collect()
}

/// Whether the body of `footprint` lies inside the board outline.
fn on_board(outline: &Polygon, footprint: &Footprint) -> Result<bool, GeomError> {
    let body = checked_body(footprint)?.aabb();
    Ok(body
        .corners()
        .into_iter()
        .all(|p| outline.contains_point(p)))
}

/// Placement: every part in scope inside its room (the rooms in scope) or
/// inside the board outline, except the parts kept where they are.
fn placement(plan: &LayoutPlan, input: &PlanInput) -> StageStatus {
    let board = input.board;
    let stage = &plan.placement;
    let kept = |fp: &Footprint| stage.keep.contains(&fp.reference);
    let mut open = Vec::new();
    let (mut total, mut placed) = (0, 0);
    if stage.rooms.is_empty() {
        let outline = match ring(&board.outline, "outline") {
            Ok(points) if points.len() >= 3 => Polygon::new(points, Vec::new()),
            _ => {
                let parts = board.footprints.iter().filter(|fp| !kept(fp)).count();
                return StageStatus {
                    stage: LayoutStage::Placement,
                    state: StageState::NotStarted,
                    reasons: vec!["the board has no outline to place parts in".into()],
                    counts: BTreeMap::from([("parts", parts), ("placed", 0)]),
                };
            }
        };
        for fp in board.footprints.iter().filter(|fp| !kept(fp)) {
            total += 1;
            match on_board(&outline, fp) {
                Ok(true) => placed += 1,
                Ok(false) => open.push(format!("{} is not inside the board outline", name(fp))),
                Err(_) => open.push(format!("{} could not be measured", name(fp))),
            }
        }
    } else {
        let named: Vec<&Room> = board
            .rooms
            .iter()
            .filter(|room| stage.rooms.contains(&room.name))
            .collect();
        for room in named {
            for fp in room.members(&board.footprints).filter(|fp| !kept(fp)) {
                total += 1;
                match rooms::overhang_nm(room, fp) {
                    Ok(0) => placed += 1,
                    Ok(_) => open.push(format!("{} is not inside room `{}`", name(fp), room.name)),
                    Err(_) => open.push(format!("{} could not be measured", name(fp))),
                }
            }
        }
    }
    judged(
        LayoutStage::Placement,
        [("parts", total), ("placed", placed)],
        placed > 0,
        open,
        "no part is in scope",
    )
}

fn name(fp: &Footprint) -> String {
    if fp.reference.is_empty() {
        format!("part {}", fp.uuid)
    } else {
        fp.reference.clone()
    }
}

/// Routing: every net in scope joined by copper; the nets the plan skips
/// are out of scope.
fn routing(plan: &LayoutPlan, input: &PlanInput) -> StageStatus {
    let board = input.board;
    let stage = &plan.routing;
    let skipped = |net: &str| stage.skip.iter().any(|s| s.eq_ignore_ascii_case(net));
    let in_order = |net: &str| {
        stage.nets.iter().any(|target| match target {
            RouteTarget::Net(name) => name.eq_ignore_ascii_case(net),
            RouteTarget::Class(class) => input
                .net_classes
                .iter()
                .any(|c| &c.name == class && c.contains_net(net)),
        })
    };
    let reports: Vec<_> = net_reports(board)
        .into_iter()
        .filter(|r| !skipped(&r.name))
        .filter(|r| {
            if stage.nets.is_empty() {
                r.pads >= 2
            } else {
                in_order(&r.name)
            }
        })
        .collect();
    let routed = reports.iter().filter(|r| r.is_routed()).count();
    let begun = reports.iter().any(|r| r.tracks + r.vias > 0);
    let open = reports
        .iter()
        .filter(|r| !r.is_routed())
        .map(|r| format!("net `{}` has {} connections to route", r.name, r.unrouted))
        .collect();
    judged(
        LayoutStage::Routing,
        [("nets", reports.len()), ("routed", routed)],
        begun,
        open,
        "no net is in scope",
    )
}

/// The pour `id`: a polygon pour of the board, not a cutout.
fn pour_of(board: &PcbBoard, id: uuid::Uuid) -> Option<&Zone> {
    board
        .zones
        .iter()
        .find(|zone| zone.uuid == id && zone.is_pour() && !zone.is_pour_cutout())
}

/// Pours: every pour in scope filled; with none named, every pour that is
/// not shelved.
fn pours(plan: &LayoutPlan, input: &PlanInput) -> StageStatus {
    let board = input.board;
    let scope: Vec<&Zone> = if plan.pours.pours.is_empty() {
        board
            .zones
            .iter()
            .filter(|z| z.is_pour() && !z.is_pour_cutout() && !z.is_shelved_pour())
            .collect()
    } else {
        plan.pours
            .pours
            .iter()
            .filter_map(|id| pour_of(board, *id))
            .collect()
    };
    let filled = scope.iter().filter(|z| z.is_filling_pour()).count();
    let open = scope
        .iter()
        .filter(|z| !z.is_filling_pour())
        .map(|z| {
            let why = if z.is_shelved_pour() {
                "is shelved"
            } else {
                "is not filled"
            };
            format!("pour {} ({}, {}) {why}", z.uuid, z.net_name, z.layer)
        })
        .collect();
    judged(
        LayoutStage::Pours,
        [("pours", scope.len()), ("filled", filled)],
        filled > 0,
        open,
        "the board has no pour in scope",
    )
}

/// Checks: the board's current DRC run has no open finding of the rules
/// named (every rule when none is), and no requirement is named, since the
/// requirements are checked by `check_requirements`, which this does not
/// run.
fn checks(plan: &LayoutPlan, input: &PlanInput) -> StageStatus {
    let stage = &plan.checks;
    let mut counts = BTreeMap::from([("findings", 0), ("waived", 0)]);
    let mut reasons: Vec<String> = stage
        .rules
        .iter()
        .filter(|id| !is_known_rule(id))
        .map(|id| format!("rule `{id}` is not a DRC rule of this build"))
        .collect();
    let unknown_rule = !reasons.is_empty();
    let Some(report) = input.drc else {
        reasons.push("the board has no DRC run since its last change; run run_drc".into());
        let state = if unknown_rule {
            StageState::Failing
        } else {
            StageState::NotStarted
        };
        return StageStatus {
            stage: LayoutStage::Checks,
            state,
            reasons,
            counts,
        };
    };
    let in_scope = |rule: &str| stage.rules.is_empty() || stage.rules.iter().any(|r| r == rule);
    let mut open: BTreeMap<&str, usize> = BTreeMap::new();
    let waivers = &input.board.options.drc_waivers;
    for v in report.violations.iter().filter(|v| in_scope(rule_id(v))) {
        match waiver_state(waivers, v, input.board) {
            WaiverState::Waived(_) => *counts.entry("waived").or_default() += 1,
            WaiverState::Lapsed(_) | WaiverState::Open => {
                *counts.entry("findings").or_default() += 1;
                *open.entry(rule_id(v)).or_default() += 1;
            }
        }
    }
    for (rule, n) in &open {
        reasons.push(format!("{rule}: {n} open findings"));
    }
    for id in &stage.requirements {
        reasons.push(format!(
            "requirement `{id}` is checked by check_requirements, which layout_plan_status does not run"
        ));
    }
    let state = if !open.is_empty() || unknown_rule {
        StageState::Failing
    } else if !stage.requirements.is_empty() {
        StageState::NotStarted
    } else {
        StageState::Done
    };
    StageStatus {
        stage: LayoutStage::Checks,
        state,
        reasons: capped(reasons),
        counts,
    }
}

#[cfg(test)]
#[path = "layout_plan_tests.rs"]
mod tests;
