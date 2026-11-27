//! One routing action as the app runs it: legs routed into a draft from a
//! pad (or a ratsnest line's end), the draft checked against the board and
//! landed as one undo step. Shared by the soak and the bench.

use pcb_geom::route::{NewCopper, RouteWorld, rule_legs, shove_run};
use pcb_geom::{CopperItem, PointNm, collect_copper, copper_layers, layer_bit, mm_to_nm, ratsnest};
use signex_drc::DrcRules;
use signex_engine::pcb::BoardEngine;
use signex_engine::pcb::draw::TrackStyle;
use signex_engine::pcb::placement::grid_step_mm;
use signex_engine::pcb::route::{
    LegRequest, RouteDraft, RouteMode, RouteRules, SEARCH_BUDGET, draft_fits, route_leg,
};
use signex_types::corner::{CornerMode, CornerRule};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::NetClass;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use crate::rng::Rng;

/// What one route action asks: the clicked points, in order, on `layer`.
#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub points: Vec<Point>,
    pub layer: CopperLayer,
    pub mode: RouteMode,
    pub corner: CornerRule,
}

/// A route action's end: its draft landed, or every leg was refused.
pub enum Routed {
    Landed(RouteDraft),
    Refused,
}

/// The rules a board is routed and checked by.
pub fn rules_of(board: &PcbBoard, classes: &[NetClass]) -> DrcRules {
    DrcRules::for_board(board, classes).expect("the board's rules")
}

/// A route to plan on `board`: most often a ratsnest line, else a stub
/// from a pad to a point near it; `legs` clicks (1 or 2). `None` on a
/// board with no netted pad.
pub fn plan(board: &PcbBoard, rng: &mut Rng, mode: RouteMode, legs: usize) -> Option<RoutePlan> {
    let copper = collect_copper(board);
    let usable = usable_layers(board);
    let corner = random_corner(rng);
    let (from, to, layers) = match ratsnest_ends(board, &copper, rng) {
        Some(found) if found.2 & usable != 0 && rng.chance(70) => found,
        _ => stub_ends(&copper, rng)?,
    };
    let layer = *rng.pick(&copper_layers(layers & usable))?;
    let mut points = vec![from.to_mm()];
    if legs > 1 {
        let (a, b) = (from.to_mm(), to.to_mm());
        points.push(Point::new(
            round_mm((a.x + b.x) / 2.0 + rng.offset_mm(2.0, 0.05)),
            round_mm((a.y + b.y) / 2.0 + rng.offset_mm(2.0, 0.05)),
        ));
    }
    points.push(to.to_mm());
    Some(RoutePlan {
        points,
        layer,
        mode,
        corner,
    })
}

/// A corner rule as a route or a drag takes it: one of the four corner
/// modes, flipped half the time.
pub fn random_corner(rng: &mut Rng) -> CornerRule {
    const MODES: [CornerMode; 4] = [
        CornerMode::Ortho90,
        CornerMode::Angle45Start,
        CornerMode::Angle45End,
        CornerMode::FreeAngle,
    ];
    CornerRule {
        mode: MODES[rng.below(MODES.len())],
        is_flipped: rng.chance(50),
    }
}

/// A random ratsnest line's ends and the layers where both hold its net.
fn ratsnest_ends(
    board: &PcbBoard,
    copper: &[CopperItem],
    rng: &mut Rng,
) -> Option<(PointNm, PointNm, u32)> {
    let lines = ratsnest(board);
    let line = rng.pick(&lines)?;
    let under = |at: PointNm| {
        copper
            .iter()
            .filter(|c| c.net == line.net && c.shape.contains(at, 0))
            .fold(0, |mask, c| mask | c.layers)
    };
    Some((line.from, line.to, under(line.from) & under(line.to)))
}

/// A random netted pad's centre and a point up to 4 mm from it.
fn stub_ends(copper: &[CopperItem], rng: &mut Rng) -> Option<(PointNm, PointNm, u32)> {
    let pads: Vec<&CopperItem> = copper.iter().filter(|c| c.pad_center.is_some()).collect();
    let pad = *rng.pick(&pads)?;
    let from = pad.pad_center?;
    let at = from.to_mm();
    let to = Point::new(
        round_mm(at.x + rng.offset_mm(4.0, 0.05)),
        round_mm(at.y + rng.offset_mm(4.0, 0.05)),
    );
    Some((from, PointNm::from_mm(to, "stub end").ok()?, pad.layers))
}

/// The copper layers a route may take: top, bottom, and the inner layers
/// the board's tracks already use.
fn usable_layers(board: &PcbBoard) -> u32 {
    board
        .segments
        .iter()
        .filter_map(|s| match parse_pcb_layer(&s.layer) {
            Some(PcbLayer::Copper(layer)) => Some(1 << layer_bit(layer)),
            _ => None,
        })
        .fold(
            1 << layer_bit(CopperLayer::Top) | 1 << layer_bit(CopperLayer::Bottom),
            |mask, bit| mask | bit,
        )
}

fn round_mm(mm: f64) -> f64 {
    (mm * 1000.0).round() / 1000.0
}

/// Route `plan` on the engine's board and land the draft as one step.
/// `Err` names an action that broke the router's own promises: a draft it
/// just routed that does not fit the board it was routed on, a draft the
/// engine refuses, or a track off the routed net.
pub fn route(
    engine: &mut BoardEngine,
    rules: &DrcRules,
    classes: &[NetClass],
    plan: &RoutePlan,
    ids: &mut impl FnMut() -> Uuid,
) -> Result<Routed, String> {
    let board = engine.board();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = RouteRules {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        corner: plan.corner,
        mode: plan.mode,
        grid_mm: grid_step_mm(board),
        budget: SEARCH_BUDGET,
    };
    let style =
        TrackStyle::starting_at(board, plan.points[0], plan.layer).with_class_width(board, classes);
    let mut draft = RouteDraft::default();
    for pair in plan.points.windows(2) {
        let leg = LegRequest {
            from: pair[0],
            to: pair[1],
            style,
            via: None,
        };
        match route_leg(board, &draft, leg, &route, &mut *ids) {
            Ok(leg) => draft.add(leg),
            Err(_) => break,
        }
    }
    if draft.is_empty() {
        return Ok(Routed::Refused);
    }
    if let Some(stray) = draft.tracks.iter().find(|t| t.net != style.net) {
        return Err(format!(
            "a routed track is off net {}: {stray:?}",
            style.net
        ));
    }
    draft_fits(board, &draft, &clearance, route.reach)
        .map_err(|e| format!("a draft just routed does not fit its board: {e}"))?;
    engine
        .execute_batch(draft.commands())
        .map_err(|e| format!("the engine refused a fitting draft: {e}"))?;
    Ok(Routed::Landed(draft))
}

/// How the push of a leg ended: the rounds run, whether it cleared the way,
/// and whether it came to an answer at all (a push that cannot be made is
/// an answer: the router walks around; running out of rounds is not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShoveTry {
    pub rounds: usize,
    pub cleared: bool,
    pub settled: bool,
}

/// The push of `plan`'s first leg on `board`, as the router tries it
/// (`pushed_way` in `signex_engine::pcb::route`): the corner rule, then the
/// flipped rule, skipping a rule with no legs. The rounds are those of the
/// attempt the router keeps; when both give up, the longer one's, settled
/// only when both settled. `None` when neither rule has legs, or a point
/// does not convert.
pub fn shove_rounds(
    board: &PcbBoard,
    rules: &DrcRules,
    classes: &[NetClass],
    plan: &RoutePlan,
) -> Option<ShoveTry> {
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let world = RouteWorld::with_reach(board, &clearance, rules.widest_clearance());
    let style =
        TrackStyle::starting_at(board, plan.points[0], plan.layer).with_class_width(board, classes);
    let a = PointNm::from_mm(*plan.points.first()?, "from").ok()?;
    let b = PointNm::from_mm(*plan.points.get(1)?, "to").ok()?;
    let radius = mm_to_nm(style.width_mm, "width").ok()?.abs() / 2;
    let layers = 1 << layer_bit(plan.layer);
    let mut gave_up: Option<ShoveTry> = None;
    for rule in [plan.corner, plan.corner.flipped()] {
        let Some(bends) = rule_legs(rule, a, b) else {
            continue;
        };
        let mut path = vec![a];
        path.extend(bends);
        let legs: Vec<NewCopper> = path
            .windows(2)
            .map(|leg| NewCopper::track(leg[0], leg[1], radius, style.net, layers))
            .collect();
        let run = shove_run(&world, &legs);
        if run.pushed.is_some() {
            return Some(ShoveTry {
                rounds: run.rounds,
                cleared: true,
                settled: run.settled,
            });
        }
        let earlier = gave_up.unwrap_or(ShoveTry {
            rounds: 0,
            cleared: false,
            settled: true,
        });
        gave_up = Some(ShoveTry {
            rounds: earlier.rounds.max(run.rounds),
            cleared: false,
            settled: earlier.settled && run.settled,
        });
    }
    gave_up
}
