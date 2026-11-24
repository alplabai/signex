//! Caner, 2026-11-24 (#2879): edges of regions and the board shape,
//! footprint drawings as lines, intersections of every curve, and Current
//! Layer for every kind of object.

use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A 100 x 80 mm board; a top region (40, 40)-(60, 60); silk circles at
/// (20, 20) and (28, 20) of radius 5; a top track x = 20 from y 10 to 30,
/// one x = 50 from y 30 to 70, one y = 70 from x 90 to 110; U1 at (70, 20)
/// with a silk line 2 mm above it; texts on silk at (5, 5), on top copper at
/// (8, 5).
fn board() -> PcbBoard {
    let track = |n: u128, a: (f64, f64), b: (f64, f64)| {
        serde_json::json!({
            "uuid": id(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
            "width": 0.25, "layer": "TopCopper"
        })
    };
    let circle = |x: f64| {
        serde_json::json!({
            "graphic_type": "circle", "layer": "TopSilkscreen", "width": 0.1,
            "center": {"x": x, "y": 20.0}, "radius": 5.0
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 100.0, "y": 0.0},
                    {"x": 100.0, "y": 80.0}, {"x": 0.0, "y": 80.0}],
        "zones": [{"uuid": id(2), "layer": "TopCopper",
                   "outline": [{"x": 40.0, "y": 40.0}, {"x": 60.0, "y": 40.0},
                               {"x": 60.0, "y": 60.0}, {"x": 40.0, "y": 60.0}]}],
        "graphics": [circle(20.0), circle(28.0)],
        "segments": [
            track(3, (20.0, 10.0), (20.0, 30.0)),
            track(4, (50.0, 30.0), (50.0, 70.0)),
            track(5, (90.0, 70.0), (110.0, 70.0))
        ],
        "footprints": [{
            "uuid": id(6), "position": {"x": 70.0, "y": 20.0}, "layer": "TopCopper",
            "rotation": 0.0, "pads": [],
            "graphics": [{"graphic_type": "line", "layer": "TopSilkscreen", "width": 0.1,
                          "start": {"x": -1.0, "y": -2.0}, "end": {"x": 1.0, "y": -2.0}}]
        }],
        "texts": [
            {"uuid": id(7), "text": "A", "position": {"x": 5.0, "y": 5.0},
             "layer": "TopSilkscreen"},
            {"uuid": id(8), "text": "B", "position": {"x": 8.0, "y": 5.0},
             "layer": "TopCopper"}
        ]
    }))
    .unwrap()
}

fn snap_on(kind: SnapObject, layers: SnapLayers, at: Point) -> Option<Point> {
    let objects: BTreeSet<SnapObject> = [kind].into();
    let skip = BTreeSet::new();
    let query = SnapQuery {
        objects: &objects,
        layers,
        skip: &skip,
        radius_mm: 0.6,
        shown: ShownLayers::ALL,
        net: None,
    };
    object_snap_point(&board(), at, &query)
}

fn snap(kind: SnapObject, at: Point) -> Option<Point> {
    snap_on(kind, SnapLayers::All, at)
}

fn close(a: Option<Point>, b: Point) -> bool {
    a.is_some_and(|a| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6)
}

const SILK: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);

#[test]
fn a_region_pulls_to_the_nearest_point_of_its_edge_and_its_vertices() {
    let hit = snap(SnapObject::Regions, Point::new(50.0, 40.3));
    assert!(close(hit, Point::new(50.0, 40.0)), "edge: {hit:?}");
    let hit = snap(SnapObject::Regions, Point::new(39.8, 39.7));
    assert!(close(hit, Point::new(40.0, 40.0)), "vertex: {hit:?}");
    assert_eq!(snap(SnapObject::Regions, Point::new(50.0, 50.0)), None);
}

#[test]
fn the_board_shape_pulls_to_the_nearest_point_of_its_edge() {
    let hit = snap(SnapObject::BoardShape, Point::new(30.0, 79.6));
    assert!(close(hit, Point::new(30.0, 80.0)), "edge: {hit:?}");
    let hit = snap(SnapObject::BoardShape, Point::new(100.2, -0.3));
    assert!(close(hit, Point::new(100.0, 0.0)), "corner: {hit:?}");
}

#[test]
fn a_footprint_drawing_line_is_a_track_arc_line() {
    let hit = snap(SnapObject::TrackLines, Point::new(70.4, 17.6));
    assert!(close(hit, Point::new(70.4, 18.0)), "{hit:?}");
}

#[test]
fn a_line_crosses_an_arc_at_an_intersection() {
    let hit = snap(SnapObject::Intersections, Point::new(20.3, 15.2));
    assert!(close(hit, Point::new(20.0, 15.0)), "{hit:?}");
}

#[test]
fn two_circles_cross_at_an_intersection() {
    let hit = snap(SnapObject::Intersections, Point::new(24.2, 17.3));
    assert!(close(hit, Point::new(24.0, 17.0)), "{hit:?}");
}

#[test]
fn a_track_crosses_a_region_edge_and_the_board_edge() {
    let hit = snap(SnapObject::Intersections, Point::new(50.2, 59.7));
    assert!(close(hit, Point::new(50.0, 60.0)), "region edge: {hit:?}");
    let hit = snap(SnapObject::Intersections, Point::new(99.7, 70.3));
    assert!(close(hit, Point::new(100.0, 70.0)), "board edge: {hit:?}");
}

/// Axes line up with fixed points only: the nearest point of a line would
/// line the pointer up with itself (implementation note, #2879).
#[test]
fn axes_ignore_the_nearest_point_of_a_line() {
    let objects: BTreeSet<SnapObject> = [SnapObject::TrackLines].into();
    let skip = BTreeSet::new();
    let query = SnapQuery {
        objects: &objects,
        layers: SnapLayers::All,
        skip: &skip,
        radius_mm: 0.6,
        shown: ShownLayers::ALL,
        net: None,
    };
    let at = Point::new(50.3, 50.0);
    assert!(object_snap_point(&board(), at, &query).is_some());
    assert_eq!(axis_snap(&board(), at, &query, 5.0), (None, None));
    let vertices: BTreeSet<SnapObject> = [SnapObject::Regions].into();
    let query = SnapQuery {
        objects: &vertices,
        ..query
    };
    // The region's corner (60, 60) is 3.2 mm away: y lines up with it.
    let at = Point::new(57.5, 59.8);
    assert_eq!(axis_snap(&board(), at, &query, 5.0), (None, Some(60.0)));
}

#[test]
fn current_layer_keeps_texts_drawings_and_regions_on_it() {
    let silk = SnapLayers::Current(SILK);
    let top = SnapLayers::Current(TOP);
    let silk_text = Point::new(5.0, 5.0);
    let copper_text = Point::new(8.0, 5.0);
    let near = |p: Point| Point::new(p.x + 0.2, p.y + 0.2);
    assert!(close(
        snap_on(SnapObject::Texts, silk, near(silk_text)),
        silk_text
    ));
    assert_eq!(snap_on(SnapObject::Texts, top, near(silk_text)), None);
    assert!(close(
        snap_on(SnapObject::Texts, top, near(copper_text)),
        copper_text
    ));
    assert_eq!(snap_on(SnapObject::Texts, silk, near(copper_text)), None);
    // A silk drawing counts on silk, not on top copper.
    let line = Point::new(70.4, 17.6);
    assert!(snap_on(SnapObject::TrackLines, silk, line).is_some());
    assert_eq!(snap_on(SnapObject::TrackLines, top, line), None);
    // The top copper region counts on top copper, not on silk.
    let edge = Point::new(50.0, 40.3);
    assert!(snap_on(SnapObject::Regions, top, edge).is_some());
    assert_eq!(snap_on(SnapObject::Regions, silk, edge), None);
}
