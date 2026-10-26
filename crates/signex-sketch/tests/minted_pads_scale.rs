//! GH #1055 — minted pads add free points no constraint touches. Those stay
//! out of the solve, so a footprint's worth of pads with one constraint
//! solves at the size of that constraint, not of every pad.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::solver::Solver;
use signex_sketch::solver::lm::DEFAULT_WORK_BUDGET;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::state::point_xy;
use signex_sketch::solver::work::WorkMeter;

/// A rectangular pad as minted: four corners, four edges, a centre point.
fn rect_pad(s: &mut Sketch, cx: f64, cy: f64) -> SketchEntityId {
    let (hw, hh) = (0.4, 0.15);
    let corners = [
        s.add_point(cx - hw, cy - hh),
        s.add_point(cx + hw, cy - hh),
        s.add_point(cx + hw, cy + hh),
        s.add_point(cx - hw, cy + hh),
    ];
    for i in 0..4 {
        s.add_line(corners[i], corners[(i + 1) % 4]);
    }
    s.add_point(cx, cy)
}

/// A rounded-rect pad as minted: four corner arcs (centre, start, end),
/// four straight edges joining them, a centre point.
fn rounded_pad(s: &mut Sketch, cx: f64, cy: f64) -> SketchEntityId {
    let (hw, hh, r) = (0.5, 0.3, 0.1);
    let mut ends = Vec::new();
    for (k, (sx, sy)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .into_iter()
        .enumerate()
    {
        let (ax, ay) = (cx + sx * (hw - r), cy + sy * (hh - r));
        let a0 = std::f64::consts::FRAC_PI_2 * k as f64;
        let a1 = a0 + std::f64::consts::FRAC_PI_2;
        let center = s.add_point(ax, ay);
        let start = s.add_point(ax + r * a0.cos(), ay + r * a0.sin());
        let end = s.add_point(ax + r * a1.cos(), ay + r * a1.sin());
        s.add_arc(center, start, end, true);
        ends.push((start, end));
    }
    for i in 0..4 {
        s.add_line(ends[i].1, ends[(i + 1) % 4].0);
    }
    s.add_point(cx, cy)
}

fn solve_with_one_distance(
    pads: usize,
    mint: fn(&mut Sketch, f64, f64) -> SketchEntityId,
) -> (Sketch, Vec<SketchEntityId>, u64) {
    let mut s = Sketch::new();
    let centres: Vec<SketchEntityId> = (0..pads)
        .map(|i| mint(&mut s, (i % 10) as f64 * 1.27, (i / 10) as f64 * 2.0))
        .collect();
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1: centres[0],
            p2: centres[1],
            target: DimTarget::Literal(1.5),
        },
    });
    let mut meter = WorkMeter::new(DEFAULT_WORK_BUDGET);
    let out = Solver::default()
        .solve_with_meter(&s.data, &ResolvedParams::new(), &mut meter)
        .unwrap_or_else(|e| panic!("{pads} pads: {e:?}"));
    // Only the two constrained centres are variables.
    assert_eq!(out.result.state.len(), 4, "{pads} pads");
    let at = |id| point_xy(id, &out.result.state, &out.result.index, &s.data).unwrap();
    let (a, b) = (at(centres[0]), at(centres[1]));
    assert!(((b.0 - a.0).hypot(b.1 - a.1) - 1.5).abs() < 1e-6);
    (s, centres, meter.spent())
}

#[test]
fn a_qfn40_of_minted_pads_with_one_constraint_solves_well_under_budget() {
    // Before #1055: n = 410 and BudgetExceeded (needed ~207M).
    let (s, centres, spent) = solve_with_one_distance(41, rect_pad);
    assert!(spent < DEFAULT_WORK_BUDGET / 1000, "spent {spent}");
    // Untouched pads keep their minted coordinates.
    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("solves");
    let third = point_xy(centres[2], &out.result.state, &out.result.index, &s.data).unwrap();
    assert_eq!(third, (2.0 * 1.27, 0.0));
}

#[test]
fn rounded_rect_pads_with_one_constraint_solve_well_under_budget() {
    // Before #1055: 17 rounded-rect pads (n = 578) ran out of the work
    // budget (BudgetExceeded, ~200M of 125M).
    let (_, _, spent) = solve_with_one_distance(17, rounded_pad);
    assert!(spent < DEFAULT_WORK_BUDGET / 1000, "spent {spent}");
}

/// A frozen arc whose closure error is above the solver tolerance (1e-12)
/// but under the freeze threshold must not leave a residual floor: the
/// sketch converges.
#[test]
fn slightly_open_untouched_arc_does_not_stall_the_solve() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(3.0, 0.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Literal(2.0),
        },
    });
    // Radius 2 start, end 1e-10 mm off the circle.
    let center = s.add_point(10.0, 0.0);
    let start = s.add_point(12.0, 0.0);
    let end = s.add_point(10.0, 2.0 + 1e-10);
    s.add_arc(center, start, end, true);
    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("a 1e-10 mm closure error in an untouched arc must converge");
    assert_eq!(out.result.state.len(), 4, "arc stays out of the solve");
}
