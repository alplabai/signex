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

fn snap(objects: &[SnapObject], layers: u32, at: Point) -> Option<Point> {
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

#[test]
fn a_point_within_the_radius_pulls_the_pointer() {
    let on = SnapObject::DEFAULT_ON;
    assert_eq!(
        snap(&on, u32::MAX, Point::new(12.2, 10.3)),
        Some(Point::new(12.0, 10.0))
    );
    assert_eq!(
        snap(&on, u32::MAX, Point::new(19.8, 10.1)),
        Some(Point::new(20.0, 10.0))
    );
    assert_eq!(snap(&on, u32::MAX, Point::new(15.0, 10.0)), None);
}

#[test]
fn a_kind_turned_off_is_not_snapped_to() {
    assert_eq!(
        snap(&[SnapObject::PadCenters], u32::MAX, Point::new(19.8, 10.1)),
        None
    );
}

#[test]
fn the_current_layer_leaves_other_layers_out() {
    let top = 1 << pcb_geom::layer_bit(CopperLayer::Top);
    let vertices = [SnapObject::TrackVertices];
    assert_eq!(snap(&vertices, top, Point::new(30.1, 10.0)), None);
    let bottom = 1 << pcb_geom::layer_bit(CopperLayer::Bottom);
    assert_eq!(
        snap(&vertices, bottom, Point::new(30.1, 10.0)),
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
        layers: u32::MAX,
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
