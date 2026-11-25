use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// U1 at (10, 10) with a top pad 2 mm right of its origin; a through via at
/// (20, 10); a bottom-layer track from (30, 10) to (40, 10).
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "footprints": [{
            "uuid": id(2), "position": {"x": 10.0, "y": 10.0}, "layer": "TopCopper",
            "pads": [{
                "uuid": id(3), "number": "1", "pad_type": "smd", "shape": "rect",
                "position": {"x": 2.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
                "drill": null, "layers": ["TopCopper"]
            }]
        }],
        "vias": [{"uuid": id(4), "position": {"x": 20.0, "y": 10.0},
                  "diameter": 0.6, "drill": 0.3}],
        "segments": [{"uuid": id(5), "start": {"x": 30.0, "y": 10.0},
                      "end": {"x": 40.0, "y": 10.0}, "width": 0.25,
                      "layer": "BottomCopper"}]
    }))
    .unwrap()
}

fn current(layer: CopperLayer) -> SnapLayers {
    SnapLayers::Current(PcbLayer::Copper(layer))
}

fn snap(objects: &[SnapObject], layers: SnapLayers, at: Point) -> Option<Point> {
    let objects: BTreeSet<SnapObject> = objects.iter().copied().collect();
    let skip = BTreeSet::new();
    let query = SnapQuery {
        objects: &objects,
        layers,
        skip: &skip,
        radius_mm: 0.5,
        shown: ShownLayers::ALL,
        net: None,
    };
    object_snap_point(&board(), at, &query)
}

/// Caner, 2026-11-24 (#2879): a new session snaps to every supported kind;
/// 3D Body Snap Points stays off until boards hold bodies.
#[test]
fn every_supported_kind_is_on_by_default() {
    let supported: Vec<SnapObject> = SnapObject::ALL
        .into_iter()
        .filter(|kind| kind.is_supported())
        .collect();
    assert_eq!(SnapObject::DEFAULT_ON.to_vec(), supported);
    assert!(!SnapObject::DEFAULT_ON.contains(&SnapObject::Body3dPoints));
}

#[test]
fn a_point_within_the_radius_pulls_the_pointer() {
    // Centres only: with every kind on (the 2026-11-24 default) the pad's
    // edge, nearer this pointer, would win.
    let on = [SnapObject::PadCenters, SnapObject::ViaCenters];
    let all = SnapLayers::All;
    assert_eq!(
        snap(&on, all, Point::new(12.2, 10.3)),
        Some(Point::new(12.0, 10.0))
    );
    assert_eq!(
        snap(&on, all, Point::new(19.8, 10.1)),
        Some(Point::new(20.0, 10.0))
    );
    assert_eq!(snap(&on, all, Point::new(15.0, 10.0)), None);
}

#[test]
fn a_kind_turned_off_is_not_snapped_to() {
    assert_eq!(
        snap(
            &[SnapObject::PadCenters],
            SnapLayers::All,
            Point::new(19.8, 10.1)
        ),
        None
    );
}

#[test]
fn the_current_layer_leaves_other_layers_out() {
    let vertices = [SnapObject::TrackVertices];
    let top = current(CopperLayer::Top);
    assert_eq!(snap(&vertices, top, Point::new(30.1, 10.0)), None);
    assert_eq!(
        snap(
            &vertices,
            current(CopperLayer::Bottom),
            Point::new(30.1, 10.0)
        ),
        Some(Point::new(30.0, 10.0))
    );
    // A through via reaches every layer.
    assert!(snap(&[SnapObject::ViaCenters], top, Point::new(20.0, 10.2)).is_some());
}

#[test]
fn a_carried_footprint_does_not_snap_to_itself() {
    let objects: BTreeSet<SnapObject> = [SnapObject::PadCenters].into_iter().collect();
    let skip: BTreeSet<Uuid> = [id(2)].into_iter().collect();
    let query = SnapQuery {
        objects: &objects,
        layers: SnapLayers::All,
        skip: &skip,
        radius_mm: 0.5,
        shown: ShownLayers::ALL,
        net: None,
    };
    assert_eq!(
        object_snap_point(&board(), Point::new(12.0, 10.0), &query),
        None
    );
}
