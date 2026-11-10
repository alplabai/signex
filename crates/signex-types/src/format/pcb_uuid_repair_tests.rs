//! Board-open integrity (#1886): a repeated object uuid is repaired.

use super::tests::empty_board;
use super::*;
use crate::pcb::{
    BoardText, Footprint, Pad, PadShape, PadType, PcbBoard, Point as PcbPoint, Segment, Via,
    ViaType, Zone,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pt(x: f64) -> PcbPoint {
    PcbPoint { x, y: 0.0 }
}

fn track(n: u128, end_x: f64) -> Segment {
    Segment {
        uuid: id(n),
        start: pt(0.0),
        end: pt(end_x),
        width: 0.2,
        layer: "TopCopper".into(),
        net: 0,
        unknown_columns: Default::default(),
    }
}

fn via(n: u128, x: f64) -> Via {
    Via {
        uuid: id(n),
        position: pt(x),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 0,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    }
}

fn zone(n: u128, name: &str) -> Zone {
    Zone {
        uuid: id(n),
        net: 0,
        net_name: name.into(),
        layer: "TopCopper".into(),
        outline: vec![pt(0.0), pt(1.0)],
        holes: Vec::new(),
        priority: 0,
        fill_type: "solid".into(),
        thermal_relief: true,
        thermal_gap: 0.5,
        thermal_width: 0.5,
        clearance: 0.2,
        min_thickness: 0.2,
        unknown: Default::default(),
    }
}

fn text(n: u128, body: &str) -> BoardText {
    BoardText {
        uuid: id(n),
        text: body.into(),
        position: pt(0.0),
        layer: "TopSilk".into(),
        font_size: 1.0,
        rotation: 0.0,
    }
}

fn pad(n: u128, number: &str) -> Pad {
    Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: id(n),
        number: number.into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: pt(0.0),
        size: pt(1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
    }
}

fn footprint(n: u128, reference: &str, pads: Vec<Pad>) -> Footprint {
    Footprint {
        schematic_link: None,
        library_ref: None,
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: id(n),
        reference: reference.into(),
        value: String::new(),
        footprint_id: "x.snxfpt".into(),
        position: pt(0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
    }
}

fn reopen(board: &PcbBoard) -> PcbBoard {
    let text = SnxPcb::new(board.clone()).write_string().unwrap();
    SnxPcb::parse(&text).unwrap().board
}

fn board() -> PcbBoard {
    let mut board = empty_board();
    board.uuid = id(1);
    board
}

#[test]
fn a_unique_board_has_nothing_to_repair() {
    let mut b = board();
    b.segments = vec![track(10, 1.0), track(11, 2.0)];
    b.vias = vec![via(12, 1.0)];
    let loaded = reopen(&b);
    assert!(loaded.unknown.load_repairs.is_empty());
}

#[test]
fn an_exact_copy_of_a_track_is_dropped() {
    let mut b = board();
    b.segments = vec![track(10, 1.0), track(10, 1.0)];
    let mut loaded = reopen(&b);
    assert_eq!(loaded.segments.len(), 1);
    let repairs = loaded.take_load_repairs();
    assert!(matches!(repairs[..], [SheetLoadRepair::CopyDropped { .. }]));
    assert!(loaded.take_load_repairs().is_empty());
}

#[test]
fn a_different_track_with_a_taken_id_gets_a_new_one() {
    let mut b = board();
    b.segments = vec![track(10, 1.0), track(10, 2.0)];
    let loaded = reopen(&b);
    assert_eq!(loaded.segments.len(), 2);
    assert_eq!(loaded.segments[0].uuid, id(10));
    assert_ne!(loaded.segments[1].uuid, id(10));
    assert_eq!(loaded.segments[1].end.x, 2.0);
    // The new id is the same on every parse of the same file.
    assert_eq!(reopen(&b).segments[1].uuid, loaded.segments[1].uuid);
}

#[test]
fn every_kind_shares_one_id_space() {
    let mut b = board();
    b.footprints = vec![footprint(20, "R1", vec![pad(21, "1")])];
    b.segments = vec![track(20, 1.0)];
    b.vias = vec![via(21, 1.0)];
    b.zones = vec![zone(22, "GND")];
    b.texts = vec![text(22, "hello"), text(1, "board id")];
    let loaded = reopen(&b);
    let mut all: Vec<Uuid> = loaded.footprints.iter().map(|f| f.uuid).collect();
    all.extend(
        loaded
            .footprints
            .iter()
            .flat_map(|f| f.pads.iter().map(|p| p.uuid)),
    );
    all.extend(loaded.segments.iter().map(|s| s.uuid));
    all.extend(loaded.vias.iter().map(|v| v.uuid));
    all.extend(loaded.zones.iter().map(|z| z.uuid));
    all.extend(loaded.texts.iter().map(|t| t.uuid));
    all.push(loaded.uuid);
    let unique: std::collections::HashSet<_> = all.iter().collect();
    assert_eq!(unique.len(), all.len());
    assert_eq!(loaded.unknown.load_repairs.len(), 4);
}

#[test]
fn a_pad_copied_onto_another_footprint_is_kept_with_a_new_id() {
    let mut b = board();
    b.footprints = vec![
        footprint(20, "R1", vec![pad(30, "1")]),
        footprint(21, "R2", vec![pad(30, "1")]),
    ];
    let loaded = reopen(&b);
    assert_eq!(loaded.footprints[1].pads.len(), 1);
    assert_ne!(loaded.footprints[1].pads[0].uuid, id(30));
    assert_eq!(loaded.footprints[0].pads[0].uuid, id(30));
}

#[test]
fn a_reminted_pad_names_its_footprint_not_a_sheet_symbol() {
    let mut b = board();
    b.footprints = vec![
        footprint(20, "R1", vec![pad(30, "1")]),
        footprint(21, "R2", vec![pad(30, "1")]),
    ];
    let mut loaded = reopen(&b);
    let text = loaded.take_load_repairs()[0].describe();
    assert!(text.contains("on footprint R2"), "{text}");
    assert!(!text.contains("sheet symbol"), "{text}");
}

#[test]
fn a_footprint_carrying_the_board_id_is_not_silently_kept() {
    let mut b = board();
    b.footprints = vec![footprint(1, "R1", vec![pad(30, "1")])];
    let mut loaded = reopen(&b);
    assert_ne!(loaded.footprints[0].uuid, loaded.uuid);
    assert_eq!(loaded.footprints[0].pads.len(), 1);
    let repairs = loaded.take_load_repairs();
    assert!(matches!(
        repairs[..],
        [SheetLoadRepair::UuidReminted { ref object, .. }] if object.kind == "footprint"
    ));
    // The pad stays attached to the footprint across another save and read.
    assert_eq!(reopen(&loaded).footprints[0].pads.len(), 1);
}
