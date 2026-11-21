//! #2374: the snap kinds, guides and axes added after #2353.

use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_options::{BoardGuide, GuideKind};
use uuid::Uuid;

use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// U1 at (10, 10) with a 2 x 1 mm rect pad on its origin; two crossing top
/// tracks, (30, 0)-(30, 20) and (20, 10)-(40, 10); a board circle at
/// (60, 10) of radius 5.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "footprints": [{
            "uuid": id(2), "position": {"x": 10.0, "y": 10.0}, "layer": "TopCopper",
            "pads": [{
                "uuid": id(3), "number": "1", "pad_type": "smd", "shape": "rect",
                "position": {"x": 0.0, "y": 0.0}, "size": {"x": 2.0, "y": 1.0},
                "drill": null, "layers": ["TopCopper"]
            }]
        }],
        "segments": [
            {"uuid": id(5), "start": {"x": 30.0, "y": 0.0}, "end": {"x": 30.0, "y": 20.0},
             "width": 0.25, "layer": "TopCopper"},
            {"uuid": id(6), "start": {"x": 20.0, "y": 10.0}, "end": {"x": 40.0, "y": 10.0},
             "width": 0.25, "layer": "TopCopper"}
        ],
        "graphics": [{"graphic_type": "circle", "layer": "TopSilkscreen", "width": 0.1,
                      "center": {"x": 60.0, "y": 10.0}, "radius": 5.0}]
    }))
    .unwrap()
}

fn query<'a>(objects: &'a BTreeSet<SnapObject>, skip: &'a BTreeSet<Uuid>) -> SnapQuery<'a> {
    SnapQuery {
        objects,
        layers: u32::MAX,
        skip,
        radius_mm: 0.6,
        shown: ShownLayers::ALL,
        net: None,
        route_layer: u32::MAX,
    }
}

fn snap(kind: SnapObject, at: Point) -> Option<Point> {
    let objects: BTreeSet<SnapObject> = [kind].into();
    let skip = BTreeSet::new();
    object_snap_point(&board(), at, &query(&objects, &skip))
}

fn close(a: Option<Point>, b: Point) -> bool {
    a.is_some_and(|a| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6)
}

#[test]
fn every_kind_but_bodies_is_supported() {
    for kind in SnapObject::ALL {
        assert_eq!(kind.is_supported(), kind != SnapObject::Body3dPoints);
    }
}

#[test]
fn a_track_line_pulls_to_its_nearest_point() {
    let hit = snap(SnapObject::TrackLines, Point::new(30.4, 4.0));
    assert!(close(hit, Point::new(30.0, 4.0)), "{hit:?}");
    // The circle's edge counts as a line too.
    let hit = snap(SnapObject::TrackLines, Point::new(65.3, 10.0));
    assert!(close(hit, Point::new(65.0, 10.0)), "{hit:?}");
    assert_eq!(snap(SnapObject::TrackLines, Point::new(25.0, 4.0)), None);
}

#[test]
fn two_tracks_cross_at_an_intersection() {
    let hit = snap(SnapObject::Intersections, Point::new(30.3, 10.2));
    assert!(close(hit, Point::new(30.0, 10.0)), "{hit:?}");
}

#[test]
fn an_arc_centre_pulls_on_every_layer_only() {
    let hit = snap(SnapObject::ArcCenters, Point::new(60.2, 10.1));
    assert!(close(hit, Point::new(60.0, 10.0)), "{hit:?}");
    let objects: BTreeSet<SnapObject> = [SnapObject::ArcCenters].into();
    let skip = BTreeSet::new();
    let top_only = SnapQuery {
        layers: 1,
        ..query(&objects, &skip)
    };
    assert_eq!(
        object_snap_point(&board(), Point::new(60.2, 10.1), &top_only),
        None
    );
}

#[test]
fn pad_vertices_and_edges() {
    let hit = snap(SnapObject::PadVertices, Point::new(11.2, 10.3));
    assert!(close(hit, Point::new(11.0, 10.5)), "{hit:?}");
    let hit = snap(SnapObject::PadEdges, Point::new(10.2, 10.9));
    assert!(close(hit, Point::new(10.2, 10.5)), "{hit:?}");
}

#[test]
fn axes_line_the_pointer_up_with_a_point_in_range() {
    let objects: BTreeSet<SnapObject> = [SnapObject::PadCenters].into();
    let skip = BTreeSet::new();
    let q = query(&objects, &skip);
    // 4 mm right of the pad, 0.2 mm below its row: y lines up.
    assert_eq!(
        axis_snap(&board(), Point::new(14.0, 10.2), &q, 5.0),
        (None, Some(10.0))
    );
    // Out of range: nothing.
    assert_eq!(
        axis_snap(&board(), Point::new(18.0, 10.2), &q, 5.0),
        (None, None)
    );
}

#[test]
fn guides_pull_to_a_crossing_before_a_line() {
    let v = BoardGuide::new(id(1), GuideKind::Vertical, Point::new(5.0, 0.0));
    let h = BoardGuide::new(id(2), GuideKind::Horizontal, Point::new(0.0, 8.0));
    let mut off = BoardGuide::new(id(3), GuideKind::Horizontal, Point::new(0.0, 3.0));
    off.enabled = false;
    let guides = [v, h, off];
    assert_eq!(
        guide_snap(&guides, Point::new(5.2, 7.7), 0.5),
        Some(GuideHit::Crossing(Point::new(5.0, 8.0)))
    );
    assert_eq!(
        guide_snap(&guides, Point::new(5.3, 1.0), 0.5),
        Some(GuideHit::Line {
            kind: GuideKind::Vertical,
            point: Point::new(5.0, 1.0)
        })
    );
    // A disabled guide pulls nothing.
    assert_eq!(guide_snap(&guides, Point::new(1.0, 3.1), 0.5), None);
}

/// U1 at (10, 10) with a net 1 pad 3 mm off its origin and, over the origin,
/// a top-only pad of net 2.
fn board_with_covered_origin() -> PcbBoard {
    let pad = |uuid: u128, x: f64, net: u32| {
        serde_json::json!({
            "uuid": id(uuid), "number": "1", "pad_type": "smd", "shape": "rect",
            "position": {"x": x, "y": 0.0}, "size": {"x": 2.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"],
            "net": {"number": net, "name": format!("N{net}")}
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "footprints": [{
            "uuid": id(2), "position": {"x": 10.0, "y": 10.0}, "layer": "TopCopper",
            "pads": [pad(3, 3.0, 1), pad(4, 0.0, 2)]
        }]
    }))
    .unwrap()
}

#[test]
fn a_foreign_pad_refuses_an_origin_only_on_the_routes_layer() {
    use signex_types::pcb_layer::CopperLayer;
    let board = board_with_covered_origin();
    let objects: BTreeSet<SnapObject> = [SnapObject::FootprintOrigins].into();
    let skip = BTreeSet::new();
    let on = |layer: CopperLayer| SnapQuery {
        net: Some(1),
        route_layer: 1 << pcb_geom::layer_bit(layer),
        ..query(&objects, &skip)
    };
    let at = Point::new(10.2, 10.0);
    assert_eq!(object_snap_point(&board, at, &on(CopperLayer::Top)), None);
    let hit = object_snap_point(&board, at, &on(CopperLayer::Bottom));
    assert!(close(hit, Point::new(10.0, 10.0)), "{hit:?}");
}

/// The cover check of an origin looks at the pads around it, not the board.
#[test]
fn only_pads_near_the_point_are_examined() {
    let mut board = board_with_covered_origin();
    let far = board.footprints[0].clone();
    for n in 0..100 {
        let mut copy = far.clone();
        copy.uuid = id(1000 + n);
        copy.position = Point::new(100.0 + n as f64 * 10.0, 100.0);
        board.footprints.push(copy);
    }
    let at = pcb_geom::PointNm::from_mm(Point::new(10.0, 10.0), "test").unwrap();
    let mut seen = 0;
    let found = crate::pcb::snap_points::any_pad_near(&board, at, |_, _| {
        seen += 1;
        false
    });
    assert!(!found);
    assert_eq!(seen, 1, "only the pad over the point");
}
