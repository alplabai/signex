//! mockup 179d — the outline checks: a clean board passes, and each failure names
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
        CheckState::NotChecked(NotChecked::NeedsRouterDiameter),
        "the router tool diameter has no source yet"
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
    let Finding::OpenEnd { edges, gap_mm, at } = &found[0] else {
        panic!("an open end: {found:?}");
    };
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
    for point in [start, end] {
        s.data.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Fixed { point },
        });
    }
    // 50 mm apart, held at 10: over-constrained.
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1: start,
            p2: end,
            target: DimTarget::Literal(10.0),
        },
    });
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
