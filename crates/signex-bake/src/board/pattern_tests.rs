//! #2517: a pattern repeats every board target, or the bake refuses it.

use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{
    BoardCutoutAttr, KeepoutAttr, KeepoutKinds, MountingHoleAttr, PourAttr, ThermalRelief,
    VScoreHintAttr, VScoreSide,
};
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::{PatternMirror, SketchData};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::{CopperLayer, MechanicalLayer, PcbLayer};
use signex_types::sketch::board_attr::{ComponentAnchorAttr, MechanicalAttr};
use uuid::Uuid;

use super::*;
use crate::array::MAX_ARRAY_INSTANCES;

const U1: Uuid = Uuid::from_u128(0x0001_0001);

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

    fn line(&mut self, a: (f64, f64), b: (f64, f64)) -> SketchEntityId {
        let (start, end) = (self.point(a.0, a.1), self.point(b.0, b.1));
        self.add(EntityKind::Line { start, end })
    }

    fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<SketchEntityId> {
        let c = [
            self.point(x0, y0),
            self.point(x1, y0),
            self.point(x1, y1),
            self.point(x0, y1),
        ];
        (0..4)
            .map(|i| {
                self.add(EntityKind::Line {
                    start: c[i],
                    end: c[(i + 1) % 4],
                })
            })
            .collect()
    }

    fn circle(&mut self, x: f64, y: f64, radius: f64) -> SketchEntityId {
        let center = self.point(x, y);
        self.add(EntityKind::Circle { center, radius })
    }

    fn get(&mut self, id: SketchEntityId) -> &mut Entity {
        self.data.entities.iter_mut().find(|e| e.id == id).unwrap()
    }

    fn pattern(&mut self, kind: ArrayKind) -> ArrayId {
        let id = ArrayId::new();
        self.data.arrays.push(Array {
            id,
            kind,
            numbering: NumberingScheme::default(),
        });
        id
    }

    fn mirror(&mut self, pattern: ArrayId, line: SketchEntityId) {
        self.data
            .add_pattern_mirror(PatternMirror {
                id: ArrayId::new(),
                pattern,
                line,
            })
            .unwrap();
    }

    fn bake(&self) -> Result<BakedBoard, BoardBakeError> {
        bake_board(&self.data, &[])
    }
}

fn row(source: SketchEntityId, count: &str, dx: &str, dy: &str) -> ArrayKind {
    ArrayKind::Linear {
        source,
        count_expr: count.into(),
        dx_expr: dx.into(),
        dy_expr: dy.into(),
    }
}

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::nil(),
        "nets": [{"number": 4, "name": "GND"}],
        "footprints": [{
            "uuid": U1,
            "reference": "U1",
            "position": {"x": 0.0, "y": 0.0},
            "layer": "TopCopper",
        }],
    }))
    .expect("a board")
}

fn keepout() -> KeepoutAttr {
    KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::ALL_COPPER,
    }
}

fn cutout() -> BoardCutoutAttr {
    BoardCutoutAttr {
        edge_radius_expr: Some("0.5mm".into()),
        through: true,
    }
}

fn pour() -> PourAttr {
    PourAttr {
        layer: PcbLayer::Copper(CopperLayer::Bottom),
        net: Some("GND".into()),
        fill_type: Default::default(),
        thermal_relief: ThermalRelief::default(),
        clearance_expr: None,
        min_thickness_expr: None,
        priority: 2,
    }
}

fn v_score() -> VScoreHintAttr {
    VScoreHintAttr {
        depth_fraction_expr: "0.25".into(),
        min_web_expr: None,
        side: VScoreSide::Top,
    }
}

fn lowest_x(ring: &[Point]) -> i64 {
    let low = ring.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    (low * 1e6).round() as i64
}

fn lowest_xs<'a>(rings: impl Iterator<Item = &'a Vec<Point>>) -> Vec<i64> {
    let mut xs: Vec<i64> = rings.map(|ring| lowest_x(ring)).collect();
    xs.sort_unstable();
    xs
}

fn um(values: &[f64]) -> Vec<i64> {
    values.iter().map(|v| (v * 1e6).round() as i64).collect()
}

fn signed_area(ring: &[Point]) -> f64 {
    (0..ring.len())
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

fn close(a: Option<Point>, x: f64, y: f64) -> bool {
    a.is_some_and(|p| (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9)
}

#[test]
fn a_patterned_keepout_writes_every_instance_with_its_attributes() {
    let mut s = Sketch::new();
    let edges = s.rect(0.0, 0.0, 2.0, 1.0);
    s.get(edges[0]).keepout = Some(keepout());
    s.pattern(row(edges[0], "3", "5mm", "0mm"));

    let keepouts = s.bake().unwrap().keepouts;

    assert_eq!(
        lowest_xs(keepouts.iter().map(|k| &k.boundary)),
        um(&[0.0, 5.0, 10.0])
    );
    assert!(
        keepouts
            .iter()
            .all(|k| k.forbids == KeepoutKinds::ALL_COPPER)
    );
    assert_eq!(
        keepouts.iter().filter(|k| k.uuid == edges[0].0).count(),
        1,
        "the source keeps its uuid"
    );
}

#[test]
fn a_pattern_of_a_corner_point_repeats_the_whole_cutout() {
    let mut s = Sketch::new();
    let edges = s.rect(0.0, 0.0, 2.0, 1.0);
    s.get(edges[1]).board_cutout = Some(cutout());
    let EntityKind::Line { start, .. } = s.get(edges[0]).kind else {
        unreachable!("a rectangle edge is a line");
    };
    s.pattern(row(start, "2", "5mm", "0mm"));

    let cutouts = s.bake().unwrap().cutouts;

    assert_eq!(
        lowest_xs(cutouts.iter().map(|c| &c.boundary)),
        um(&[0.0, 5.0])
    );
    assert!(cutouts.iter().all(|c| (c.edge_radius - 0.5).abs() < 1e-12));
    assert!(cutouts.iter().all(|c| c.boundary.len() == 4));
}

#[test]
fn a_patterned_v_score_writes_every_line() {
    let mut s = Sketch::new();
    let score = s.line((0.0, 0.0), (50.0, 0.0));
    s.get(score).v_score = Some(v_score());
    s.pattern(row(score, "3", "0mm", "10mm"));

    let scores = s.bake().unwrap().v_scores;

    let mut ys: Vec<i64> = scores
        .iter()
        .map(|v| {
            assert_eq!(v.from.y, v.to.y);
            (v.from.y * 1e6).round() as i64
        })
        .collect();
    ys.sort_unstable();
    assert_eq!(ys, um(&[0.0, 10.0, 20.0]));
    assert!(scores.iter().all(|v| v.depth_fraction == 0.25));
}

#[test]
fn a_patterned_pour_writes_a_zone_per_instance_on_its_net() {
    let mut s = Sketch::new();
    let edges = s.rect(0.0, 0.0, 2.0, 1.0);
    s.get(edges[0]).pour = Some(pour());
    s.pattern(row(edges[0], "2", "5mm", "0mm"));

    let zones = bake_board_items(&s.data, &board()).unwrap().zones;

    assert_eq!(lowest_xs(zones.iter().map(|z| &z.outline)), um(&[0.0, 5.0]));
    assert!(zones.iter().all(|z| z.net == 4 && z.priority == 2));
    assert_ne!(zones[0].uuid, zones[1].uuid);
}

#[test]
fn a_patterned_mechanical_drawing_writes_every_edge() {
    let mut s = Sketch::new();
    let line = s.line((0.0, 0.0), (4.0, 0.0));
    s.get(line).mechanical = Some(MechanicalAttr::default());
    let ring = s.circle(0.0, 10.0, 2.0);
    s.get(ring).mechanical = Some(MechanicalAttr::default());
    s.pattern(row(line, "2", "5mm", "0mm"));
    s.pattern(row(ring, "2", "5mm", "0mm"));

    let graphics = bake_board_items(&s.data, &board()).unwrap().graphics;

    assert_eq!(graphics.len(), 4);
    let circles: Vec<_> = graphics
        .iter()
        .filter(|g| g.kind() == Some(BoardGraphicKind::Circle))
        .collect();
    assert_eq!(circles.len(), 2);
    assert!(
        circles
            .iter()
            .all(|c| c.start.is_none() && (c.radius - 2.0).abs() < 1e-9)
    );
    assert!(circles.iter().any(|c| close(c.center, 5.0, 10.0)));
    assert!(circles.iter().any(|c| close(c.end, 7.0, 10.0)));
}

#[test]
fn a_mirrored_mechanical_arc_stays_counter_clockwise() {
    let mut s = Sketch::new();
    let (c, a, b) = (s.point(0.0, 0.0), s.point(5.0, 0.0), s.point(0.0, 5.0));
    let arc = s.add(EntityKind::Arc {
        center: c,
        start: a,
        end: b,
        sweep_ccw: true,
    });
    s.get(arc).mechanical = Some(MechanicalAttr::default());
    let axis = s.line((10.0, -5.0), (10.0, 5.0));
    let single = s.pattern(row(arc, "1", "1mm", "0mm"));
    s.mirror(single, axis);

    let graphics = bake_board_items(&s.data, &board()).unwrap().graphics;

    assert_eq!(graphics.len(), 2);
    let image = graphics.iter().find(|g| g.uuid != arc.0).unwrap();
    assert!(close(image.center, 20.0, 0.0), "{image:?}");
    assert!(close(image.start, 20.0, 5.0), "{image:?}");
    assert!(close(image.end, 15.0, 0.0), "{image:?}");
}

/// A closed mechanical rectangle, and its corner Point for a pattern to name.
fn mechanical_rect(s: &mut Sketch) -> (Vec<SketchEntityId>, SketchEntityId) {
    let edges = s.rect(0.0, 0.0, 2.0, 1.0);
    for &edge in &edges {
        s.get(edge).mechanical = Some(MechanicalAttr::default());
    }
    let EntityKind::Line { start, .. } = s.get(edges[0]).kind else {
        unreachable!("a rectangle edge is a line");
    };
    (edges, start)
}

#[test]
fn a_pattern_of_one_corner_repeats_the_whole_closed_mechanical_shape() {
    let mut s = Sketch::new();
    let (edges, corner) = mechanical_rect(&mut s);
    s.pattern(row(corner, "3", "5mm", "0mm"));

    let graphics = bake_board_items(&s.data, &board()).unwrap().graphics;

    assert_eq!(graphics.len(), 12, "four edges at three places");
    let ids: std::collections::HashSet<_> = graphics.iter().map(|g| g.uuid).collect();
    assert_eq!(ids.len(), 12, "each instance has its own uuid");
    assert!(edges.iter().all(|edge| ids.contains(&edge.0)));
    for x in [0.0, 5.0, 10.0] {
        let at_place = graphics
            .iter()
            .filter(|g| {
                g.start
                    .is_some_and(|p| p.x >= x - 1e-9 && p.x <= x + 2.0 + 1e-9)
            })
            .count();
        assert_eq!(at_place, 4, "the whole rectangle at x = {x}");
    }
}

#[test]
fn a_closed_mechanical_shape_turned_onto_itself_writes_once() {
    let mut s = Sketch::new();
    let (edges, _) = mechanical_rect(&mut s);
    let centre = s.point(1.0, 0.5);
    s.pattern(ArrayKind::Polar {
        source: edges[0],
        center: centre,
        count_expr: "2".into(),
        sweep_angle_expr: "360deg".into(),
        depopulation: None,
    });

    let graphics = bake_board_items(&s.data, &board()).unwrap().graphics;

    assert_eq!(graphics.len(), 4, "a half turn lands on the same rectangle");
}

/// An open chain repeats as a whole too; an edge it does not touch, or one
/// touching it on another layer, is another shape.
#[test]
fn a_pattern_repeats_the_connected_open_chain_on_its_layer_only() {
    let mut s = Sketch::new();
    let (a, b, c) = (s.point(0.0, 0.0), s.point(4.0, 0.0), s.point(4.0, 3.0));
    let first = s.add(EntityKind::Line { start: a, end: b });
    let second = s.add(EntityKind::Line { start: b, end: c });
    let other_layer = s.add(EntityKind::Line { start: b, end: a });
    let apart = s.line((0.0, 20.0), (4.0, 20.0));
    for edge in [first, second, apart] {
        s.get(edge).mechanical = Some(MechanicalAttr::default());
    }
    s.get(other_layer).mechanical = Some(MechanicalAttr {
        layer: MechanicalLayer::new(2).unwrap(),
    });
    s.pattern(row(c, "2", "10mm", "0mm"));

    let graphics = bake_board_items(&s.data, &board()).unwrap().graphics;

    let written = |source: SketchEntityId| graphics.iter().filter(|g| g.uuid == source.0).count();
    assert_eq!(graphics.len(), 6, "2 x 2 chain edges, and the two others");
    assert_eq!((written(first), written(second)), (1, 1));
    assert_eq!((written(other_layer), written(apart)), (1, 1));
}

#[test]
fn baking_a_patterned_closed_mechanical_shape_twice_gives_the_same_board() {
    let mut s = Sketch::new();
    let (_, corner) = mechanical_rect(&mut s);
    s.pattern(row(corner, "3", "5mm", "0mm"));
    let board = board();

    let first = bake_board_with_items(&s.data, &board).unwrap();
    let second = bake_board_with_items(&s.data, &board).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.1.graphics.len(), 12);
}

#[test]
fn a_patterned_outline_is_refused_by_name() {
    let mut s = Sketch::new();
    let edges = s.rect(0.0, 0.0, 50.0, 30.0);
    for &edge in &edges {
        s.get(edge).board_outline = true;
    }
    s.pattern(row(edges[2], "2", "60mm", "0mm"));

    let error = s.bake().unwrap_err();

    assert!(
        matches!(
            error,
            BoardBakeError::NotPatternable {
                kind: BakedKind::BoardOutline,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(error.place().is_some(), "the refusal selects the outline");
}

#[test]
fn a_patterned_component_anchor_is_refused_by_name() {
    let mut s = Sketch::new();
    let at = s.point(1.0, 1.0);
    s.get(at).component_anchor = Some(ComponentAnchorAttr {
        footprint: Some(U1),
    });
    s.pattern(row(at, "2", "5mm", "0mm"));

    let error = bake_board_items(&s.data, &board()).unwrap_err();

    assert!(
        matches!(
            error,
            BoardBakeError::NotPatternable {
                kind: BakedKind::ComponentAnchors,
                entity,
            } if entity == at
        ),
        "{error:?}"
    );
}

#[test]
fn a_pattern_of_a_pattern_and_its_mirror_repeat_a_keepout_with_its_winding() {
    let mut s = Sketch::new();
    let edges = s.rect(5.0, 5.0, 7.0, 6.0);
    s.get(edges[0]).keepout = Some(keepout());
    let axis = s.line((25.0, 0.0), (25.0, 30.0));
    let inner = s.pattern(row(edges[0], "2", "5mm", "0mm"));
    let outer = s.pattern(row(edges[0], "2", "0mm", "10mm"));
    s.data.set_pattern_source(outer, inner).unwrap();
    s.mirror(outer, axis);

    let keepouts = s.bake().unwrap().keepouts;

    assert_eq!(keepouts.len(), 8, "2 x 2, and the mirror of each");
    assert_eq!(
        lowest_xs(keepouts.iter().map(|k| &k.boundary)),
        um(&[5.0, 5.0, 10.0, 10.0, 38.0, 38.0, 43.0, 43.0])
    );
    let source = keepouts.iter().find(|k| k.uuid == edges[0].0).unwrap();
    let winding = signed_area(&source.boundary).signum();
    assert!(
        keepouts
            .iter()
            .all(|k| signed_area(&k.boundary).signum() == winding),
        "a reflection keeps the boundary's winding"
    );
}

/// Every image shares the pivot corner, so a key of one point would keep
/// only the source.
#[test]
fn a_polar_pattern_about_its_own_corner_keeps_every_instance() {
    let mut s = Sketch::new();
    let edges = s.rect(0.0, 0.0, 2.0, 1.0);
    s.get(edges[0]).board_cutout = Some(cutout());
    let EntityKind::Line { start, .. } = s.get(edges[0]).kind else {
        unreachable!("a rectangle edge is a line");
    };
    s.pattern(ArrayKind::Polar {
        source: edges[0],
        center: start,
        count_expr: "4".into(),
        sweep_angle_expr: "360deg".into(),
        depopulation: None,
    });

    assert_eq!(s.bake().unwrap().cutouts.len(), 4);
}

#[test]
fn a_circle_turned_about_its_centre_is_one_keepout() {
    let mut s = Sketch::new();
    let disc = s.circle(5.0, 5.0, 2.0);
    s.get(disc).keepout = Some(keepout());
    let EntityKind::Circle { center, .. } = s.get(disc).kind else {
        unreachable!("the keepout is a circle");
    };
    s.pattern(ArrayKind::Polar {
        source: disc,
        center,
        count_expr: "7".into(),
        sweep_angle_expr: "360deg".into(),
        depopulation: None,
    });

    let keepouts = s.bake().unwrap().keepouts;

    assert_eq!(keepouts.len(), 1);
    assert_eq!(keepouts[0].uuid, disc.0);
}

#[test]
fn two_patterns_on_one_source_share_the_instance_cap() {
    let each = (MAX_ARRAY_INSTANCES / 2 + 1).to_string();
    let mut s = Sketch::new();
    let hole = s.circle(0.0, 0.0, 1.0);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    s.pattern(row(hole, &each, "5mm", "0mm"));
    s.pattern(row(hole, &each, "0mm", "5mm"));

    match s.bake() {
        Err(BoardBakeError::TooManyInstances { entity, limit }) => {
            assert_eq!((entity, limit), (hole, MAX_ARRAY_INSTANCES));
        }
        other => panic!("expected the cap, got {other:?}"),
    }
}

#[test]
fn patterns_on_different_targets_share_the_instance_cap() {
    let each = (MAX_ARRAY_INSTANCES / 2 + 1).to_string();
    let mut s = Sketch::new();
    let hole = s.circle(0.0, 0.0, 1.0);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    let score = s.line((0.0, 5.0), (10.0, 5.0));
    s.get(score).v_score = Some(v_score());
    s.pattern(row(hole, &each, "5mm", "0mm"));
    s.pattern(row(score, &each, "0mm", "5mm"));

    assert!(matches!(
        s.bake(),
        Err(BoardBakeError::TooManyInstances { .. })
    ));
}

#[test]
fn patterns_within_the_cap_together_bake() {
    let each = (MAX_ARRAY_INSTANCES / 2).to_string();
    let mut s = Sketch::new();
    let hole = s.circle(0.0, 0.0, 1.0);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    s.pattern(row(hole, &each, "5mm", "0mm"));
    s.pattern(row(hole, &each, "0mm", "5mm"));

    let expected = 2 * (MAX_ARRAY_INSTANCES / 2) - 1;
    assert_eq!(s.bake().unwrap().holes.len(), expected, "they share (0, 0)");
}

#[test]
fn baking_a_patterned_sketch_twice_gives_the_same_board() {
    let mut s = Sketch::new();
    let keep = s.rect(0.0, 0.0, 2.0, 1.0);
    s.get(keep[0]).keepout = Some(keepout());
    s.pattern(row(keep[0], "3", "5mm", "0mm"));
    let cut = s.rect(0.0, 10.0, 2.0, 11.0);
    s.get(cut[0]).board_cutout = Some(cutout());
    s.pattern(row(cut[0], "3", "5mm", "0mm"));
    let copper = s.rect(0.0, 20.0, 2.0, 21.0);
    s.get(copper[0]).pour = Some(pour());
    s.pattern(row(copper[0], "3", "5mm", "0mm"));
    let hole = s.circle(0.0, 30.0, 1.0);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    s.pattern(row(hole, "3", "5mm", "0mm"));
    let score = s.line((0.0, 40.0), (10.0, 40.0));
    s.get(score).v_score = Some(v_score());
    s.pattern(row(score, "3", "0mm", "5mm"));
    let drawn = s.line((0.0, 60.0), (4.0, 60.0));
    s.get(drawn).mechanical = Some(MechanicalAttr::default());
    s.pattern(row(drawn, "3", "5mm", "0mm"));
    let board = board();

    let first = bake_board_with_items(&s.data, &board).unwrap();
    let second = bake_board_with_items(&s.data, &board).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.0.keepouts.len(), 3);
    assert_eq!(first.0.cutouts.len(), 3);
    assert_eq!(first.0.holes.len(), 3);
    assert_eq!(first.0.v_scores.len(), 3);
    assert_eq!(first.1.zones.len(), 3);
    assert_eq!(first.1.graphics.len(), 3);
}

/// Mockup row 179e, #2894 / #2772: a 2 x 2 hole pattern spaced by the size
/// parameters bakes into four holes, and they follow an edit of `board_w`.
#[test]
fn a_rectangular_hole_pattern_bound_to_parameters_bakes_one_hole_per_instance() {
    let mut s = Sketch::new();
    s.data.parameters.insert("board_w", "60mm");
    s.data.parameters.insert("board_h", "40mm");
    s.data.parameters.insert("hole_inset", "3.5mm");
    let hole = s.circle(3.5, 3.5, 1.6);
    s.get(hole).mounting_hole = Some(MountingHoleAttr::default());
    s.pattern(ArrayKind::Grid {
        source: hole,
        nx_expr: "2".into(),
        ny_expr: "2".into(),
        dx_expr: "board_w - 2*hole_inset".into(),
        dy_expr: "board_h - 2*hole_inset".into(),
        depopulation: None,
    });
    let centres = |baked: &BakedBoard| {
        let mut at: Vec<(i64, i64)> = baked
            .holes
            .iter()
            .map(|h| (um(&[h.position.x])[0], um(&[h.position.y])[0]))
            .collect();
        at.sort_unstable();
        at
    };
    let grid = |xs: [f64; 2], ys: [f64; 2]| {
        let mut at: Vec<(i64, i64)> = xs
            .iter()
            .flat_map(|&x| ys.iter().map(move |&y| (um(&[x])[0], um(&[y])[0])))
            .collect();
        at.sort_unstable();
        at
    };

    let baked = s.bake().unwrap();
    assert_eq!(centres(&baked), grid([3.5, 56.5], [3.5, 36.5]));
    let mut uuids: Vec<Uuid> = baked.holes.iter().map(|h| h.uuid).collect();
    uuids.sort_unstable();
    uuids.dedup();
    assert_eq!(uuids.len(), 4, "each instance is its own hole");

    s.data.parameters.insert("board_w", "70mm");
    let wider = s.bake().unwrap();
    assert_eq!(centres(&wider), grid([3.5, 66.5], [3.5, 36.5]));
}
