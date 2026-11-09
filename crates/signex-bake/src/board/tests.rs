//! #2234 — the board bake: one test per product and per refusal.

use signex_sketch::attr::{
    BoardCutoutAttr, KeepoutAttr, KeepoutKinds, MountingHoleAttr, VScoreHintAttr, VScoreSide,
};
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::pcb::{NetDef, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::geometry::CIRCLE_VERTICES;
use super::*;
use crate::profile::ARC_SAMPLES;

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

    fn bake(&self) -> Result<BakedBoard, BoardBakeError> {
        bake_board(&self.data, &nets())
    }
}

fn nets() -> Vec<NetDef> {
    vec![
        NetDef {
            number: 1,
            name: "VCC".into(),
        },
        NetDef {
            number: 4,
            name: "GND".into(),
        },
    ]
}

fn sorted(points: &[Point]) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = points
        .iter()
        .map(|p| ((p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64))
        .collect();
    out.sort_unstable();
    out
}

fn mm(points: &[(f64, f64)]) -> Vec<Point> {
    points.iter().map(|&(x, y)| Point { x, y }).collect()
}

/// A 50 x 30 board outline.
fn board_outline(s: &mut Sketch) -> Vec<SketchEntityId> {
    let edges = s.rect(0.0, 0.0, 50.0, 30.0);
    for &edge in &edges {
        s.get(edge).board_outline = true;
    }
    edges
}

#[test]
fn a_rectangle_outline_bakes_to_its_four_corners() {
    let mut s = Sketch::new();
    board_outline(&mut s);
    let outline = s.bake().unwrap().outline.expect("an outline");
    assert_eq!(
        sorted(&outline),
        sorted(&mm(&[(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)]))
    );
}

#[test]
fn one_tagged_edge_is_enough_and_an_arc_is_tessellated() {
    let mut s = Sketch::new();
    let a = s.point(0.0, 0.0);
    let b = s.point(10.0, 0.0);
    let c = s.point(10.0, 10.0);
    let d = s.point(0.0, 10.0);
    let center = s.point(0.0, 5.0);
    let first = s.line(a, b);
    s.line(b, c);
    s.line(c, d);
    s.add(EntityKind::Arc {
        center,
        start: d,
        end: a,
        sweep_ccw: true,
    });
    s.get(first).board_outline = true;

    let outline = s.bake().unwrap().outline.unwrap();
    assert_eq!(outline.len(), 4 + ARC_SAMPLES - 1, "{outline:?}");
    let leftmost = outline.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    assert!(
        (leftmost + 5.0).abs() < 0.05,
        "the arc bulges left: {leftmost}"
    );
}

#[test]
fn a_circle_outline_is_a_round_board() {
    let mut s = Sketch::new();
    let disc = s.circle(20.0, 20.0, 15.0);
    s.get(disc).board_outline = true;
    let outline = s.bake().unwrap().outline.unwrap();
    assert_eq!(outline.len(), CIRCLE_VERTICES);
    for p in &outline {
        let r = ((p.x - 20.0).powi(2) + (p.y - 20.0).powi(2)).sqrt();
        assert!((r - 15.0).abs() < 1e-9, "{p:?}");
    }
}

#[test]
fn construction_and_centerline_entities_never_bake() {
    let mut s = Sketch::new();
    board_outline(&mut s);
    let a = s.point(0.0, 0.0);
    let b = s.point(50.0, 30.0);
    let diagonal = s.line(a, b);
    s.get(diagonal).board_outline = true;
    s.get(diagonal).construction = true;
    let guide = s.circle(10.0, 10.0, 2.0);
    s.get(guide).centerline = true;
    s.get(guide).keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::ALL_COPPER,
    });
    s.get(guide).mounting_hole = Some(MountingHoleAttr::default());

    let baked = s.bake().unwrap();
    assert_eq!(baked.outline.unwrap().len(), 4);
    assert!(baked.keepouts.is_empty());
    assert!(baked.holes.is_empty());
}

#[test]
fn without_an_outline_the_board_keeps_its_own_until_one_was_baked() {
    let mut s = Sketch::new();
    s.circle(5.0, 5.0, 1.0);
    assert_eq!(s.bake().unwrap().outline, None);

    s.data.record_baked(BakedKind::BoardOutline, true);
    assert_eq!(
        s.bake().unwrap().outline,
        Some(Vec::new()),
        "deleted, so cleared"
    );
}

#[test]
fn an_open_outline_is_refused_by_name() {
    let mut s = Sketch::new();
    let edges = board_outline(&mut s);
    s.data.entities.retain(|e| e.id != edges[2]);
    match s.bake() {
        Err(BoardBakeError::OpenProfile { kind, .. }) => {
            assert_eq!(kind, BakedKind::BoardOutline);
        }
        other => panic!("expected an open profile, got {other:?}"),
    }
}

#[test]
fn a_branching_outline_is_refused() {
    let mut s = Sketch::new();
    let edges = board_outline(&mut s);
    // At the seed edge's far end, where the walk has to pick a way on.
    let EntityKind::Line { end, .. } = s.get(edges[0]).kind else {
        unreachable!("a rectangle edge is a line");
    };
    let far = s.point(60.0, -5.0);
    s.line(end, far);
    assert!(matches!(
        s.bake(),
        Err(BoardBakeError::BranchingProfile {
            kind: BakedKind::BoardOutline,
            ..
        })
    ));
}

#[test]
fn two_outlines_are_refused() {
    let mut s = Sketch::new();
    board_outline(&mut s);
    let second = s.circle(100.0, 100.0, 5.0);
    s.get(second).board_outline = true;
    assert!(matches!(
        s.bake(),
        Err(BoardBakeError::OutlineCount { count: 2 })
    ));
}

#[test]
fn a_point_tagged_as_outline_is_not_an_edge() {
    let mut s = Sketch::new();
    let p = s.point(1.0, 1.0);
    s.get(p).board_outline = true;
    assert!(matches!(s.bake(), Err(BoardBakeError::NotAnEdge { .. })));
}

#[test]
fn a_sketch_that_does_not_solve_is_refused() {
    let mut s = Sketch::new();
    board_outline(&mut s);
    let a = s.point(1.0, 1.0);
    s.line(a, SketchEntityId::new());
    assert!(matches!(s.bake(), Err(BoardBakeError::Solve(_))));
}

#[test]
fn parameters_that_do_not_resolve_are_refused() {
    let mut s = Sketch::new();
    s.data.parameters.insert("a", "= b");
    s.data.parameters.insert("b", "= a");
    assert!(matches!(s.bake(), Err(BoardBakeError::Parameters(_))));
}

#[test]
fn a_keepout_keeps_its_kinds_layer_and_seed_uuid() {
    let mut s = Sketch::new();
    let edges = s.rect(5.0, 5.0, 10.0, 8.0);
    let kinds = KeepoutKinds {
        no_routing: true,
        no_vias: true,
        ..KeepoutKinds::default()
    };
    for &edge in &edges {
        s.get(edge).keepout = Some(KeepoutAttr {
            layer: PcbLayer::Copper(CopperLayer::Bottom),
            kinds,
        });
    }
    let disc = s.circle(30.0, 10.0, 3.0);
    s.get(disc).keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::ALL_COPPER,
    });

    let keepouts = s.bake().unwrap().keepouts;
    assert_eq!(keepouts.len(), 2, "one per loop, not per edge");
    assert_eq!(keepouts[0].uuid, edges[0].0);
    assert_eq!(keepouts[0].forbids, kinds);
    assert_eq!(
        keepouts[0].layers,
        vec![PcbLayer::Copper(CopperLayer::Bottom)]
    );
    assert_eq!(
        sorted(&keepouts[0].boundary),
        sorted(&mm(&[(5.0, 5.0), (10.0, 5.0), (10.0, 8.0), (5.0, 8.0)]))
    );
    assert_eq!(keepouts[1].uuid, disc.0);
    assert_eq!(keepouts[1].boundary.len(), CIRCLE_VERTICES);
}

#[test]
fn an_open_keepout_is_refused() {
    let mut s = Sketch::new();
    let a = s.point(0.0, 0.0);
    let b = s.point(5.0, 0.0);
    let edge = s.line(a, b);
    s.get(edge).keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::default(),
    });
    assert!(matches!(
        s.bake(),
        Err(BoardBakeError::OpenProfile {
            kind: BakedKind::Keepouts,
            ..
        })
    ));
}

#[test]
fn a_cutout_is_an_opening_with_its_edge_radius() {
    let mut s = Sketch::new();
    board_outline(&mut s);
    let edges = s.rect(20.0, 10.0, 25.0, 15.0);
    s.get(edges[1]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: Some("0.5mm".into()),
        through: false,
    });
    let cutouts = s.bake().unwrap().cutouts;
    assert_eq!(cutouts.len(), 1);
    assert_eq!(cutouts[0].uuid, edges[1].0);
    assert!((cutouts[0].edge_radius - 0.5).abs() < 1e-12);
    assert!(!cutouts[0].through);
    assert_eq!(
        sorted(&cutouts[0].boundary),
        sorted(&mm(&[
            (20.0, 10.0),
            (25.0, 10.0),
            (25.0, 15.0),
            (20.0, 15.0)
        ]))
    );
}

#[test]
fn a_cutout_radius_that_does_not_evaluate_is_refused() {
    let mut s = Sketch::new();
    let edges = s.rect(0.0, 0.0, 1.0, 1.0);
    s.get(edges[0]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: Some("= missing".into()),
        through: true,
    });
    assert!(matches!(
        s.bake(),
        Err(BoardBakeError::Expression {
            what: "cutout edge radius",
            ..
        })
    ));
}

#[test]
fn a_mounting_hole_sits_at_its_circle_with_its_drill_plating_and_net() {
    let mut s = Sketch::new();
    s.data.parameters.insert("hole_d", "2.7mm");
    let by_size = s.circle(4.0, 4.0, 1.6);
    s.get(by_size).mounting_hole = Some(MountingHoleAttr::default());
    let by_param = s.circle(46.0, 4.0, 1.0);
    s.get(by_param).mounting_hole = Some(MountingHoleAttr {
        drill_expr: Some("= hole_d".into()),
        plated: true,
        net: Some("GND".into()),
    });

    let holes = s.bake().unwrap().holes;
    assert_eq!(holes.len(), 2);
    assert_eq!(holes[0].uuid, by_size.0);
    assert_eq!(holes[0].position, Point { x: 4.0, y: 4.0 });
    assert!(
        (holes[0].drill - 3.2).abs() < 1e-12,
        "the circle's diameter"
    );
    assert!(!holes[0].plated);
    assert_eq!(holes[0].net, None);
    assert_eq!(holes[1].position, Point { x: 46.0, y: 4.0 });
    assert!((holes[1].drill - 2.7).abs() < 1e-12);
    assert!(holes[1].plated);
    assert_eq!(holes[1].net, Some(4), "GND's number");
}

#[test]
fn a_hole_naming_a_net_the_board_lacks_is_refused() {
    let mut s = Sketch::new();
    let hole = s.circle(4.0, 4.0, 1.6);
    s.get(hole).mounting_hole = Some(MountingHoleAttr {
        net: Some("CHASSIS".into()),
        ..MountingHoleAttr::default()
    });
    match s.bake() {
        Err(BoardBakeError::UnknownNet { entity, net }) => {
            assert_eq!((entity, net.as_str()), (hole, "CHASSIS"));
        }
        other => panic!("expected an unknown net, got {other:?}"),
    }
}

#[test]
fn a_hole_must_be_a_circle_with_a_positive_drill() {
    let mut s = Sketch::new();
    let a = s.point(0.0, 0.0);
    let b = s.point(1.0, 0.0);
    let line = s.line(a, b);
    s.get(line).mounting_hole = Some(MountingHoleAttr::default());
    assert!(matches!(
        s.bake(),
        Err(BoardBakeError::HoleNotACircle { .. })
    ));

    let mut s = Sketch::new();
    let hole = s.circle(4.0, 4.0, 1.6);
    s.get(hole).mounting_hole = Some(MountingHoleAttr {
        drill_expr: Some("0mm".into()),
        ..MountingHoleAttr::default()
    });
    assert!(matches!(s.bake(), Err(BoardBakeError::HoleDrill { .. })));
}

#[test]
fn a_v_score_is_a_line_with_its_depth_fraction() {
    let mut s = Sketch::new();
    let a = s.point(0.0, 15.0);
    let b = s.point(50.0, 15.0);
    let score = s.line(a, b);
    s.get(score).v_score = Some(VScoreHintAttr {
        depth_fraction_expr: "0.25".into(),
        min_web_expr: Some("0.4mm".into()),
        side: VScoreSide::Top,
    });
    let v_scores = s.bake().unwrap().v_scores;
    assert_eq!(v_scores.len(), 1);
    let v = &v_scores[0];
    assert_eq!(v.uuid, score.0);
    assert_eq!(
        (v.from, v.to),
        (Point { x: 0.0, y: 15.0 }, Point { x: 50.0, y: 15.0 })
    );
    assert_eq!(v.depth_fraction, 0.25);
    assert!((v.min_web - 0.4).abs() < 1e-12);
    assert_eq!(v.side, VScoreSide::Top);

    let mut s = Sketch::new();
    let disc = s.circle(1.0, 1.0, 1.0);
    s.get(disc).v_score = Some(VScoreHintAttr {
        depth_fraction_expr: "0.25".into(),
        min_web_expr: None,
        side: VScoreSide::Both,
    });
    assert!(matches!(
        s.bake(),
        Err(BoardBakeError::VScoreNotALine { .. })
    ));
}

#[test]
fn baking_twice_gives_the_same_board() {
    let mut s = Sketch::new();
    board_outline(&mut s);
    let hole = s.circle(4.0, 4.0, 1.6);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    assert_eq!(s.bake().unwrap(), s.bake().unwrap());
}
