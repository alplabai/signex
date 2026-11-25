//! mockup row 179d — the outline checks: a clean board passes, and each failure names
//! its place.

use signex_sketch::Solver;
use signex_sketch::attr::{BoardCutoutAttr, KeepoutAttr, KeepoutKinds, MountingHoleAttr};
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::parameter::resolve;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::pcb::Point;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::*;

struct Sketch {
    data: SketchData,
    plane: PlaneId,
}

impl Sketch {
    fn new() -> Self {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        Sketch { data, plane }
    }

    fn add(&mut self, kind: EntityKind) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data.entities.push(Entity::new(id, self.plane, kind));
        id
    }

    fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
        self.add(EntityKind::Point { x, y })
    }

    fn line(&mut self, start: SketchEntityId, end: SketchEntityId) -> SketchEntityId {
        self.add(EntityKind::Line { start, end })
    }

    fn circle(&mut self, x: f64, y: f64, radius: f64) -> SketchEntityId {
        let center = self.point(x, y);
        self.add(EntityKind::Circle { center, radius })
    }

    /// A closed polyline through `corners`, its edges tagged as outline.
    fn outline(&mut self, corners: &[(f64, f64)]) -> Vec<SketchEntityId> {
        let points: Vec<SketchEntityId> = corners.iter().map(|&(x, y)| self.point(x, y)).collect();
        let n = points.len();
        let edges: Vec<SketchEntityId> = (0..n)
            .map(|i| self.line(points[i], points[(i + 1) % n]))
            .collect();
        for &edge in &edges {
            self.get(edge).board_outline = true;
        }
        edges
    }

    /// The four edges of an axis-aligned rectangle, bottom edge first.
    fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<SketchEntityId> {
        let corners = [
            self.point(x0, y0),
            self.point(x1, y0),
            self.point(x1, y1),
            self.point(x0, y1),
        ];
        (0..4)
            .map(|i| self.line(corners[i], corners[(i + 1) % 4]))
            .collect()
    }

    fn get(&mut self, id: SketchEntityId) -> &mut Entity {
        self.data.entities.iter_mut().find(|e| e.id == id).unwrap()
    }

    fn solve(&self) -> FullSolveOutput {
        let params = resolve(&self.data.parameters).unwrap();
        Solver::default().solve(&self.data, &params).unwrap()
    }

    fn checks(&self) -> OutlineChecks {
        let solve = self.solve();
        outline_checks(&self.data, &solve, &solve.params)
    }
}

const BOARD: [(f64, f64); 4] = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)];

fn findings(state: &CheckState) -> &[Finding] {
    match state {
        CheckState::Failed(findings) => findings,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn a_clean_rectangle_with_its_openings_inside_passes() {
    let mut s = Sketch::new();
    s.outline(&BOARD);
    let hole = s.circle(5.0, 5.0, 1.6);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    let cutout = s.rect(20.0, 10.0, 30.0, 20.0);
    s.get(cutout[0]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    });
    let keepout = s.circle(40.0, 15.0, 4.0);
    s.get(keepout).keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::ALL_COPPER,
    });

    let checks = s.checks();

    assert_eq!(checks.closed, CheckState::Passed);
    assert_eq!(checks.no_crossing, CheckState::Passed);
    assert_eq!(checks.openings_inside, CheckState::Passed);
    assert_eq!(
        checks.inside_corners,
        CheckState::NotChecked(NotChecked::NoRouterTool),
        "no router tool diameter was given"
    );
    assert_eq!(checks.failures().count(), 0);
}

#[test]
fn a_sketch_without_an_outline_checks_nothing() {
    let mut s = Sketch::new();
    s.rect(0.0, 0.0, 10.0, 10.0);

    let checks = s.checks();

    for check in [OutlineCheck::Closed, OutlineCheck::NoCrossing] {
        assert_eq!(
            *checks.state(check),
            CheckState::NotChecked(NotChecked::NoOutline),
            "{check:?}"
        );
    }
}

#[test]
fn an_open_outline_names_both_open_ends_and_the_gap() {
    let mut s = Sketch::new();
    let corners: Vec<SketchEntityId> = BOARD.iter().map(|&(x, y)| s.point(x, y)).collect();
    // The left edge stops 0.02 mm short of the first corner.
    let short = s.point(0.0, 0.02);
    let bottom = s.line(corners[0], corners[1]);
    let right = s.line(corners[1], corners[2]);
    let top = s.line(corners[2], corners[3]);
    let left = s.line(corners[3], short);
    for edge in [bottom, right, top, left] {
        s.get(edge).board_outline = true;
    }

    let checks = s.checks();

    let found = findings(&checks.closed);
    assert_eq!(found.len(), 1, "{found:?}");
    let Finding::OpenEnd {
        edges,
        ends,
        gap_mm,
        at,
    } = &found[0]
    else {
        panic!("an open end: {found:?}");
    };
    let mut ends = ends.clone();
    ends.sort_by_key(|id| id.0);
    let mut open_points = vec![corners[0], short];
    open_points.sort_by_key(|id| id.0);
    assert_eq!(ends, open_points, "the two points Close gap joins");
    let mut edges = edges.clone();
    edges.sort_by_key(|id| id.0);
    let mut expected = vec![bottom, left];
    expected.sort_by_key(|id| id.0);
    assert_eq!(edges, expected, "the two edges whose ends do not meet");
    assert!((gap_mm.unwrap() - 0.02).abs() < 1e-9, "{gap_mm:?}");
    let at = at.unwrap();
    assert!(at.x.abs() < 1e-9 && (at.y - 0.01).abs() < 1e-9, "{at:?}");
    assert_eq!(found[0].place().len(), 2);
    assert_eq!(
        checks.no_crossing,
        CheckState::NotChecked(NotChecked::NeedsClosedOutline)
    );
    assert_eq!(
        checks.openings_inside,
        CheckState::NotChecked(NotChecked::NeedsClosedOutline)
    );
}

#[test]
fn two_outline_loops_fail_the_closed_check() {
    let mut s = Sketch::new();
    let first = s.outline(&BOARD);
    let second = s.outline(&[(60.0, 0.0), (70.0, 0.0), (70.0, 10.0)]);

    let checks = s.checks();

    let found = findings(&checks.closed);
    let Finding::SeveralLoops { count, seeds } = &found[0] else {
        panic!("several loops: {found:?}");
    };
    assert_eq!(*count, 2);
    assert!(seeds.contains(&first[0]) && seeds.contains(&second[0]));
}

#[test]
fn a_figure_eight_names_the_two_crossing_edges_and_the_point() {
    let mut s = Sketch::new();
    // A bow tie: the first and third edges cross at (5, 5).
    let edges = s.outline(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)]);

    let checks = s.checks();

    assert_eq!(checks.closed, CheckState::Passed, "it is one closed loop");
    let found = findings(&checks.no_crossing);
    assert_eq!(found.len(), 1, "{found:?}");
    let Finding::Crossing { edges: crossed, at } = &found[0] else {
        panic!("a crossing: {found:?}");
    };
    assert!(crossed.contains(&edges[0]) && crossed.contains(&edges[2]));
    assert!(
        (at.x - 5.0).abs() < 1e-9 && (at.y - 5.0).abs() < 1e-9,
        "{at:?}"
    );
}

#[test]
fn a_hole_outside_or_across_the_edge_is_named() {
    let mut s = Sketch::new();
    s.outline(&BOARD);
    let outside = s.circle(60.0, 10.0, 1.5);
    let across = s.circle(49.0, 10.0, 1.5);
    let inside = s.circle(10.0, 10.0, 1.5);
    for hole in [outside, across, inside] {
        s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    }

    let checks = s.checks();

    let found = findings(&checks.openings_inside);
    let named: Vec<SketchEntityId> = found.iter().map(|f| f.place()[0]).collect();
    assert_eq!(named, vec![outside, across]);
    assert!(found.iter().all(|f| matches!(
        f,
        Finding::OpeningOutside {
            kind: BakedKind::MountingHoles,
            ..
        }
    )));
}

#[test]
fn a_hole_whose_pattern_instance_leaves_the_board_is_named() {
    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};

    let mut s = Sketch::new();
    s.outline(&BOARD);
    let hole = s.circle(40.0, 10.0, 1.5);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    s.data.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: hole,
            count_expr: "3".into(),
            dx_expr: "5mm".into(),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::default(),
    });

    let checks = s.checks();

    assert_eq!(
        findings(&checks.openings_inside),
        [Finding::OpeningOutside {
            entity: hole,
            kind: BakedKind::MountingHoles,
        }]
    );
}

#[test]
fn a_cutout_crossing_the_edge_is_named_by_its_seed_edge() {
    let mut s = Sketch::new();
    s.outline(&BOARD);
    let cutout = s.rect(45.0, 10.0, 55.0, 20.0);
    s.get(cutout[1]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    });

    let checks = s.checks();

    assert_eq!(
        findings(&checks.openings_inside),
        [Finding::OpeningOutside {
            entity: cutout[1],
            kind: BakedKind::Cutouts,
        }]
    );
}

/// A sketch that does not solve has no checks; a solve that is behind an
/// edit reads as not checked. Neither panics.
#[test]
fn a_solve_error_yields_no_checks_rather_than_a_panic() {
    let mut s = Sketch::new();
    let edges = s.outline(&BOARD);
    let EntityKind::Line { start, end } = s.get(edges[0]).kind else {
        unreachable!()
    };
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: start },
    });
    // One edge held at 10 mm and at 50 mm: over-constrained. With both ends
    // fixed nothing would be free, and such a solve converges with its
    // conflict only flagged.
    for mm in [10.0, 50.0] {
        s.data.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::DistancePtPt {
                p1: start,
                p2: end,
                target: DimTarget::Literal(mm),
            },
        });
    }
    let params = resolve(&s.data.parameters).unwrap();
    let solve = Solver::default().solve(&s.data, &params);
    assert!(solve.is_err(), "the fixture is over-constrained");
    assert_eq!(outline_checks_after(&s.data, solve.ok().as_ref()), None);

    // The last good solve, then a new outline loop it never saw.
    let mut s = Sketch::new();
    s.point(1.0, 1.0);
    let stale = s.solve();
    s.outline(&BOARD);
    let checks = outline_checks(&s.data, &stale, &stale.params);
    assert_eq!(checks.closed, CheckState::NotChecked(NotChecked::Unsolved));
}

#[test]
fn a_bake_refusal_names_the_entity_it_is_about() {
    let edge = SketchEntityId::new();
    let refusal = BoardBakeError::OpenProfile {
        kind: BakedKind::BoardOutline,
        edge,
    };
    assert_eq!(refusal.place(), Some(edge));
    assert_eq!(BoardBakeError::OutlineCount { count: 2 }.place(), None);
}

// ── Inside corners (mockup row 179d, decision 3) ──────────────────────────────────

/// An L-shaped board, counter-clockwise: its one concave corner is at
/// (10, 10).
const L_BOARD: [(f64, f64); 6] = [
    (0.0, 0.0),
    (20.0, 0.0),
    (20.0, 10.0),
    (10.0, 10.0),
    (10.0, 20.0),
    (0.0, 20.0),
];

impl Sketch {
    fn checks_with_router(&self, diameter: Option<f64>) -> OutlineChecks {
        let solve = self.solve();
        outline_checks_with(&self.data, &solve, &solve.params, diameter)
    }

    fn arc(
        &mut self,
        center: SketchEntityId,
        start: SketchEntityId,
        end: SketchEntityId,
        sweep_ccw: bool,
    ) -> SketchEntityId {
        self.add(EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        })
    }

    /// [`L_BOARD`] with its concave corner rounded by a tangent arc of
    /// `radius`; returns the arc.
    fn filleted_l(&mut self, radius: f64) -> SketchEntityId {
        let r = radius;
        let p: Vec<SketchEntityId> = [
            (0.0, 0.0),
            (20.0, 0.0),
            (20.0, 10.0),
            (10.0 + r, 10.0),
            (10.0, 10.0 + r),
            (10.0, 20.0),
            (0.0, 20.0),
        ]
        .iter()
        .map(|&(x, y)| self.point(x, y))
        .collect();
        let center = self.point(10.0 + r, 10.0 + r);
        let mut edges = vec![
            self.line(p[0], p[1]),
            self.line(p[1], p[2]),
            self.line(p[2], p[3]),
        ];
        let arc = self.arc(center, p[3], p[4], false);
        edges.push(arc);
        edges.extend([
            self.line(p[4], p[5]),
            self.line(p[5], p[6]),
            self.line(p[6], p[0]),
        ]);
        for edge in edges {
            self.get(edge).board_outline = true;
        }
        arc
    }
}

/// The corner findings of `checks`, as (entity, at, radius, tool radius).
fn corners(checks: &OutlineChecks) -> Vec<(SketchEntityId, Point, f64, f64)> {
    findings(&checks.inside_corners)
        .iter()
        .map(|f| match f {
            Finding::TightCorner {
                entity,
                at,
                radius_mm,
                tool_radius_mm,
            } => (*entity, *at, *radius_mm, *tool_radius_mm),
            other => panic!("a tight corner: {other:?}"),
        })
        .collect()
}

#[test]
fn a_sharp_concave_outline_corner_is_tighter_than_the_router_either_way_round() {
    let mut reversed = L_BOARD;
    reversed.reverse();
    for board in [L_BOARD, reversed] {
        let mut s = Sketch::new();
        s.outline(&board);

        let checks = s.checks_with_router(Some(2.0));

        let found = corners(&checks);
        assert_eq!(found.len(), 1, "only the concave corner: {found:?}");
        let (entity, at, radius, tool) = found[0];
        assert!(
            (at.x - 10.0).abs() < 1e-9 && (at.y - 10.0).abs() < 1e-9,
            "{at:?}"
        );
        assert_eq!(radius, 0.0);
        assert_eq!(tool, 1.0);
        let corner = s.get(entity).kind.clone();
        assert_eq!(
            corner,
            EntityKind::Point { x: 10.0, y: 10.0 },
            "the corner point, so a click selects it"
        );
        assert_eq!(findings(&checks.inside_corners)[0].place(), [entity]);
    }
}

#[test]
fn a_convex_board_has_no_inside_corner_to_check() {
    let mut s = Sketch::new();
    s.outline(&BOARD);
    assert_eq!(
        s.checks_with_router(Some(2.0)).inside_corners,
        CheckState::Passed
    );
}

#[test]
fn a_fillet_at_least_the_tool_radius_passes_and_a_smaller_one_is_named() {
    let mut s = Sketch::new();
    s.filleted_l(1.5);
    let checks = s.checks_with_router(Some(2.0));
    assert_eq!(checks.closed, CheckState::Passed);
    assert_eq!(checks.inside_corners, CheckState::Passed, "R 1.5 >= R 1.0");

    let mut s = Sketch::new();
    let arc = s.filleted_l(0.5);
    let found = corners(&s.checks_with_router(Some(2.0)));
    assert_eq!(
        found.len(),
        1,
        "the arc only, its tangent ends are no corners: {found:?}"
    );
    let (entity, at, radius, tool) = found[0];
    assert_eq!(entity, arc);
    assert!((radius - 0.5).abs() < 1e-9, "{radius}");
    assert_eq!(tool, 1.0);
    // The middle of the arc, toward the corner it rounds.
    assert!(
        at.x < 10.5 && at.y < 10.5 && at.x > 10.0 && at.y > 10.0,
        "{at:?}"
    );
}

#[test]
fn every_corner_of_a_cutout_is_checked_and_its_edge_radius_counts() {
    let mut s = Sketch::new();
    s.outline(&BOARD);
    let cutout = s.rect(20.0, 10.0, 30.0, 20.0);
    s.get(cutout[0]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    });

    let found = corners(&s.checks_with_router(Some(2.0)));
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found.iter().all(|&(_, _, radius, _)| radius == 0.0));

    s.get(cutout[0]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: Some("1.2mm".into()),
        through: true,
    });
    assert_eq!(
        s.checks_with_router(Some(2.0)).inside_corners,
        CheckState::Passed,
        "an edge radius of 1.2 mm is cut by a 2 mm tool"
    );
}

#[test]
fn a_round_cutout_smaller_than_the_tool_is_named_and_a_drilled_hole_is_not() {
    let mut s = Sketch::new();
    s.outline(&BOARD);
    let cutout = s.circle(10.0, 10.0, 0.5);
    s.get(cutout).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    });
    let hole = s.circle(40.0, 10.0, 0.3);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());

    let found = corners(&s.checks_with_router(Some(2.0)));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, cutout);
    assert!((found[0].2 - 0.5).abs() < 1e-9);
}

#[test]
fn without_a_router_tool_or_an_outline_the_corners_are_not_checked() {
    let mut s = Sketch::new();
    s.outline(&L_BOARD);
    assert_eq!(
        s.checks_with_router(None).inside_corners,
        CheckState::NotChecked(NotChecked::NoRouterTool)
    );
    let mut s = Sketch::new();
    s.rect(0.0, 0.0, 10.0, 10.0);
    assert_eq!(
        s.checks_with_router(Some(2.0)).inside_corners,
        CheckState::NotChecked(NotChecked::NoOutline)
    );
}

/// Decision 3: every outline check warns only. A board whose inside corner
/// fails still bakes.
#[test]
fn a_failing_corner_check_does_not_stop_the_bake() {
    let mut s = Sketch::new();
    s.outline(&L_BOARD);
    assert_eq!(corners(&s.checks_with_router(Some(2.0))).len(), 1);
    let baked = crate::board::bake_board(&s.data, &[]).expect("the board still bakes");
    assert!(baked.outline.is_some_and(|o| !o.is_empty()));
}
