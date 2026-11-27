//! #2598 — a board sketch pour with corners other than Sharp writes
//! `SNXPCB_FORMAT_V1_13`, which a build that knows only 1.12 opens read-only,
//! and wins over the 1.12 shaped-pour rule of the zone it bakes. A board
//! whose sketch pours are Sharp keeps its token, byte for byte. Policy:
//! `docs/FORMAT_VERSIONS.md`.

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::{PcbBoard, Point, Zone};
use crate::pcb_layer::{CopperLayer, PcbLayer};
use crate::pcb_sketch::BoardSketch;
use crate::sketch::SketchData;
use crate::sketch::attr::{PourAttr, PourFillType, ThermalRelief};
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::SketchEntityId;
use crate::sketch::plane::{Plane, PlaneId, PlaneKind};
use crate::zone_pour::POUR_FILL;
use crate::zone_pour_shape::{PourCorners, PourFillMode};
use uuid::Uuid;

fn save(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().unwrap()
}

fn token_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("format = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// A board whose sketch holds one line between two points carrying a Sharp
/// pour.
fn board() -> PcbBoard {
    let plane = PlaneId::new();
    let (a, b) = (SketchEntityId::new(), SketchEntityId::new());
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    sketch
        .entities
        .push(Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
    sketch
        .entities
        .push(Entity::new(b, plane, EntityKind::Point { x: 5.0, y: 0.0 }));
    let mut edge = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Line { start: a, end: b },
    );
    edge.pour = Some(PourAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        net: None,
        fill_type: PourFillType::Solid,
        thermal_relief: ThermalRelief::default(),
        clearance_expr: None,
        min_thickness_expr: None,
        priority: 0,
        corners: PourCorners::Sharp,
        corner_size: 0.0,
    });
    sketch.entities.push(edge);
    let mut board = empty_board();
    board.sketch = Some(BoardSketch {
        sketch,
        ..BoardSketch::default()
    });
    board
}

fn set_corners(board: &mut PcbBoard, corners: PourCorners, size: f64) {
    let stored = board.sketch.as_mut().expect("a board sketch");
    for pour in stored
        .sketch
        .entities
        .iter_mut()
        .filter_map(|e| e.pour.as_mut())
    {
        pour.corners = corners;
        pour.corner_size = size;
    }
}

/// The policy of a build that stops at `snxpcb/1.12`.
fn one_twelve() -> FormatVersion {
    FormatVersion {
        current: SNXPCB_FORMAT_V1_12,
        readable: &[
            "snxpcb/1",
            "snxpcb/1.1",
            "snxpcb/1.2",
            "snxpcb/1.3",
            SNXPCB_FORMAT_V1,
            SNXPCB_FORMAT_LAYER_COLORS,
            SNXPCB_FORMAT_ROOMS,
            SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
            SNXPCB_FORMAT_V1_8,
            SNXPCB_FORMAT_V1_9,
            SNXPCB_FORMAT_V1_10,
            SNXPCB_FORMAT_V1_11,
            SNXPCB_FORMAT_V1_12,
        ],
        ..SNXPCB_VERSION
    }
}

#[test]
fn board_sketch_pour_corners_write_snxpcb_1_13_and_round_trip() {
    for (corners, token) in [
        (PourCorners::Rounded, "rounded"),
        (PourCorners::Chamfer, "chamfer"),
    ] {
        let mut board = board();
        set_corners(&mut board, corners, 0.5);
        let text = save(&board);
        assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_13, "{text}");
        assert!(text.contains(token), "{text}");
        let file = SnxPcb::parse(&text).unwrap();
        assert!(!file.is_from_newer_minor(), "this build reads 1.13");
        assert_eq!(file.board.sketch, board.sketch, "the corners round-trip");
        assert_eq!(file.write_string().unwrap(), text, "byte-identical re-save");
    }
}

/// The baked zone's shape needs 1.12; the sketch attribute wins with 1.13.
#[test]
fn sketch_pour_corners_win_over_the_shaped_zone_they_bake() {
    let mut board = board();
    let zone = Zone {
        uuid: Uuid::from_u128(9),
        net: 0,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.0),
            Point::new(5.0, 5.0),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        corners: PourCorners::Rounded,
        corner_size: 0.5,
        fill_mode: PourFillMode::Solid,
        hatch_width: 0.0,
        hatch_gap: 0.0,
        unknown: Default::default(),
    };
    board.zones.push(zone);
    assert!(board.zones[0].has_pour_shape());
    assert_eq!(snxpcb_token(&board), SNXPCB_FORMAT_V1_12, "the zone alone");
    set_corners(&mut board, PourCorners::Rounded, 0.5);
    assert_eq!(snxpcb_token(&board), SNXPCB_FORMAT_V1_13);
}

#[test]
fn a_board_whose_sketch_pours_are_sharp_keeps_its_token_and_bytes() {
    let mut board = board();
    let before = save(&board);
    assert_ne!(token_of(&before), SNXPCB_FORMAT_V1_13, "{before}");
    set_corners(&mut board, PourCorners::Chamfer, 1.0);
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_13);
    set_corners(&mut board, PourCorners::Sharp, 0.0);
    assert_eq!(save(&board), before, "Sharp again writes the old bytes");
}

#[test]
fn a_build_that_knows_only_1_12_opens_sketch_pour_corners_read_only() {
    let mut board = board();
    set_corners(&mut board, PourCorners::Rounded, 0.5);
    let text = save(&board);
    let token = token_of(&text);
    let older = one_twelve();
    assert!(older.is_newer_minor(token), "{token}");
    assert!(older.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
}
