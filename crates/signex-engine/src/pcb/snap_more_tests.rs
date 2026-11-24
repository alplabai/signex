//! #2374: the snap kinds, guides and axes added after #2353.

use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
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
        layers: SnapLayers::All,
        skip,
        radius_mm: 0.6,
        shown: ShownLayers::ALL,
        net: None,
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

/// Caner, 2026-11-24 (#2879): Current Layer keeps a drawing on the current
/// layer and leaves one on another out (it used to leave every drawing out).
#[test]
fn an_arc_centre_pulls_on_its_own_layer_only() {
    let hit = snap(SnapObject::ArcCenters, Point::new(60.2, 10.1));
    assert!(close(hit, Point::new(60.0, 10.0)), "{hit:?}");
    let objects: BTreeSet<SnapObject> = [SnapObject::ArcCenters].into();
    let skip = BTreeSet::new();
    let top_only = SnapQuery {
        layers: SnapLayers::Current(PcbLayer::Copper(CopperLayer::Top)),
        ..query(&objects, &skip)
    };
    assert_eq!(
        object_snap_point(&board(), Point::new(60.2, 10.1), &top_only),
        None
    );
    let silk = SnapQuery {
        layers: SnapLayers::Current(PcbLayer::Technical(TechnicalLayer::SilkscreenTop)),
        ..query(&objects, &skip)
    };
    let hit = object_snap_point(&board(), Point::new(60.2, 10.1), &silk);
    assert!(close(hit, Point::new(60.0, 10.0)), "{hit:?}");
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
