//! Decision 2 (Caner, 2026-11-20): the board sketch targets in a `.snxpcb`.
//! The new attributes live on the entities of `[board_sketch.sketch]` and
//! raise the token to `snxpcb/1.7`; what a pour or a mechanical drawing
//! bakes to is the board's own zone and drawing rows.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::pcb::{BoardGraphic, PcbBoard, Point, Zone};
use crate::pcb_layer::{CopperLayer, MechanicalLayer, PcbLayer};
use crate::pcb_options::MARK_COLORS;
use crate::pcb_sketch::BoardSketch;
use crate::sketch::SketchData;
use crate::sketch::array::ArrayId;
use crate::sketch::attr::{BoardCutoutAttr, PourAttr, PourFillType, ThermalRelief};
use crate::sketch::board_attr::{BoardSlotAttr, ComponentAnchorAttr, MechanicalAttr};
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::SketchEntityId;
use crate::sketch::pattern_links::PatternMirror;
use crate::sketch::plane::{Plane, PlaneId, PlaneKind};
use crate::zone_pour::POUR_FILL;

fn write(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().expect("write")
}

fn read(text: &str) -> PcbBoard {
    SnxPcb::parse(text).expect("parse").board
}

fn token_of(text: &str) -> &str {
    text.lines()
        .next()
        .and_then(|line| line.strip_prefix("format = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// A board whose sketch holds one line, with `tag` applied to it.
fn board_with(tag: impl FnOnce(&mut Entity)) -> PcbBoard {
    let plane = PlaneId::new();
    let (a, b) = (SketchEntityId::new(), SketchEntityId::new());
    let mut edge = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Line { start: a, end: b },
    );
    tag(&mut edge);
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    sketch.entities = vec![
        Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
        Entity::new(b, plane, EntityKind::Point { x: 10.0, y: 0.0 }),
        edge,
    ];
    let mut board = empty_board();
    board.sketch = Some(BoardSketch {
        sketch,
        ..BoardSketch::default()
    });
    board
}

fn assert_round_trips_under_1_7(board: &PcbBoard) {
    let text = write(board);
    assert_eq!(
        token_of(&text),
        SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
        "{text}"
    );
    let back = read(&text);
    assert_eq!(back.sketch, board.sketch);
    assert!(!SnxPcb::parse(&text).unwrap().is_from_newer_minor());
    assert_eq!(write(&back), text, "a second save is byte-identical");
}

#[test]
fn a_slot_round_trips_and_writes_1_7() {
    let board = board_with(|e| {
        e.board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
        e.board_slot = Some(BoardSlotAttr::default());
    });
    assert_round_trips_under_1_7(&board);
}

#[test]
fn a_board_pour_round_trips_and_writes_1_7() {
    let board = board_with(|e| {
        e.pour = Some(PourAttr {
            layer: PcbLayer::Copper(CopperLayer::Bottom),
            net: Some("GND".into()),
            fill_type: PourFillType::Solid,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: None,
            min_thickness_expr: None,
            priority: 2,
        });
    });
    assert_round_trips_under_1_7(&board);
}

#[test]
fn a_mechanical_drawing_round_trips_and_writes_1_7() {
    let board = board_with(|e| {
        e.mechanical = Some(MechanicalAttr {
            layer: MechanicalLayer::new(4).unwrap(),
        });
    });
    let text = write(&board);
    assert!(text.contains("Mechanical4"), "{text}");
    assert_round_trips_under_1_7(&board);
}

#[test]
fn a_component_anchor_round_trips_and_writes_1_7() {
    let footprint = Uuid::now_v7();
    let mut board = board_with(|_| {});
    if let Some(stored) = board.sketch.as_mut() {
        stored.sketch.entities[0].component_anchor = Some(ComponentAnchorAttr {
            footprint: Some(footprint),
        });
    }
    let text = write(&board);
    assert!(text.contains(&footprint.to_string()), "{text}");
    assert_round_trips_under_1_7(&board);
}

/// The `snxfpt/1.2` sketch lists on a board sketch need 1.7 too, and
/// round-trip; the board goes back once the last one is gone.
#[test]
fn a_board_sketch_pattern_mirror_round_trips_and_writes_1_7() {
    let mut board = board_with(|_| {});
    if let Some(stored) = board.sketch.as_mut() {
        let line = stored.sketch.entities[2].id;
        stored.sketch.pattern_mirrors.push(PatternMirror {
            id: ArrayId::new(),
            pattern: ArrayId::new(),
            line,
        });
    }
    assert_round_trips_under_1_7(&board);
    if let Some(stored) = board.sketch.as_mut() {
        stored.sketch.pattern_mirrors.clear();
    }
    assert_eq!(token_of(&write(&board)), SNXPCB_FORMAT_V1);
}

/// A board sketch without the new targets, and a board with layer colours,
/// keep their old token and their bytes.
#[test]
fn a_board_without_the_targets_keeps_1_4_or_1_5_byte_for_byte() {
    let plain = board_with(|e| e.board_outline = true);
    let text = write(&plain);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1, "{text}");
    assert_eq!(write(&read(&text)), text);

    let mut coloured = plain.clone();
    let top = PcbLayer::Copper(CopperLayer::Top);
    assert!(coloured.options.set_layer_color(top, Some(MARK_COLORS[0])));
    let text = write(&coloured);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_LAYER_COLORS, "{text}");
    assert_eq!(write(&read(&text)), text);

    assert_eq!(token_of(&write(&empty_board())), SNXPCB_FORMAT_V1);
}

/// The token follows content: the last target removed, the board goes back.
#[test]
fn removing_the_last_target_lowers_the_token_again() {
    let mut board = board_with(|e| e.mechanical = Some(MechanicalAttr::default()));
    assert_eq!(token_of(&write(&board)), SNXPCB_FORMAT_BOARD_SKETCH_TARGETS);
    if let Some(stored) = board.sketch.as_mut() {
        stored.sketch.entities[2].mechanical = None;
    }
    assert_eq!(token_of(&write(&board)), SNXPCB_FORMAT_V1);
}

#[test]
fn the_targets_token_is_read_and_current() {
    assert!(SNXPCB_VERSION.readable.contains(&"snxpcb/1.7"));
    assert_eq!(SNXPCB_VERSION.current, SNXPCB_FORMAT_BOARD_SKETCH_TARGETS);
}

/// The rooms token (`snxpcb/1.6`) is this build's own: a board with rooms
/// opens for editing and saves. The tokens rank 1.7 > 1.6 > 1.5 > 1.4, so a
/// board with rooms and a sketch target writes 1.7 and keeps its rooms.
#[test]
fn a_rooms_board_opens_and_saves_and_the_targets_token_outranks_rooms() {
    let room = crate::pcb::Room::drawn(
        Uuid::from_u128(7),
        "dcdc".into(),
        "#A058C8",
        crate::pcb::RoomOutline::Rect {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 10.0,
            h_mm: 10.0,
        },
    );
    let mut rooms_only = board_with(|e| e.board_outline = true);
    rooms_only.rooms.push(room.clone());
    let text = write(&rooms_only);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_ROOMS);
    let parsed = SnxPcb::parse(&text).expect("a rooms board opens");
    assert!(!parsed.is_from_newer_minor(), "it opens for editing");
    assert_eq!(parsed.write_string().expect("it saves"), text);

    let mut both = board_with(|e| e.mechanical = Some(MechanicalAttr::default()));
    both.rooms.push(room);
    let text = write(&both);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_BOARD_SKETCH_TARGETS);
    assert_eq!(read(&text).rooms.len(), 1, "the rooms are kept");

    for token in ["snxpcb/1.4", "snxpcb/1.5", "snxpcb/1.6", "snxpcb/1.7"] {
        assert!(!snxpcb_opens_read_only(token), "{token}");
    }
}

/// A pour and a mechanical drawing bake to the board's own rows, under the
/// uuid of the entity they came from; both read back unchanged.
#[test]
fn sketch_pour_zones_and_mechanical_graphics_round_trip_as_board_rows() {
    let pour_entity = Uuid::now_v7();
    let drawing_entity = Uuid::now_v7();
    let mut board = board_with(|_| {});
    board.zones.push(Zone {
        uuid: pour_entity,
        net: 0,
        net_name: String::new(),
        layer: "BottomCopper".into(),
        outline: vec![
            Point { x: 1.0, y: 1.0 },
            Point { x: 9.0, y: 1.0 },
            Point { x: 9.0, y: 6.5 },
        ],
        holes: Vec::new(),
        priority: 2,
        fill_type: POUR_FILL.into(),
        thermal_relief: true,
        thermal_gap: 0.5,
        thermal_width: 0.25,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        unknown: UnknownFields::default(),
    });
    board.graphics.push(BoardGraphic {
        uuid: drawing_entity,
        graphic_type: "line".into(),
        layer: "Mechanical3".into(),
        width: 0.1,
        start: Some(Point { x: 0.0, y: 0.0 }),
        end: Some(Point { x: 10.0, y: 0.0 }),
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    });
    let text = write(&board);
    let back = read(&text);
    assert_eq!(back.zones, board.zones);
    assert_eq!(back.graphics, board.graphics);
    assert_eq!(write(&back), text);
}
