//! #2580 / #2593: a pour with Thermal Relief on joins the pads of its net
//! through spokes across an air gap; off, it joins them solidly.

use signex_types::pad_thermal::PadThermal;
use signex_types::pcb::{
    Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment, Zone,
};
use signex_types::zone_pour::POUR_FILL;
use uuid::Uuid;

use super::super::zone_fills;
use crate::units::PointNm;

/// A 1 mm square SMD pad of net 1 at the footprint's origin.
fn pad(thermal: PadThermal) -> Pad {
    Pad {
        uuid: Uuid::from_u128(10),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "GND".into(),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        thermal,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// A net-1 pour over (-10,-10)..(10,10), clearance 0.5, its relief on or
/// off with a 0.5 mm gap and 0.3 mm spokes.
fn pour(relief: bool) -> Zone {
    Zone {
        uuid: Uuid::from_u128(1),
        net: 1,
        net_name: "GND".into(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(-10.0, -10.0),
            Point::new(10.0, -10.0),
            Point::new(10.0, 10.0),
            Point::new(-10.0, 10.0),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: POUR_FILL.into(),
        thermal_relief: relief,
        thermal_gap: 0.5,
        thermal_width: 0.3,
        clearance: 0.5,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        corners: Default::default(),
        corner_size: 0.0,
        fill_mode: Default::default(),
        hatch_width: 0.0,
        hatch_gap: 0.0,
        unknown: Default::default(),
    }
}

fn board(relief: bool, rotation: f64, pad: Pad) -> PcbBoard {
    let footprint = Footprint {
        uuid: Uuid::from_u128(100),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: vec![pour(relief)],
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

/// Whether the pour's fill holds copper at (x, y) mm.
fn copper_at(board: &PcbBoard, x: f64, y: f64) -> bool {
    let at = PointNm::from_mm(Point::new(x, y), "test").unwrap();
    zone_fills(board)
        .of(0)
        .expect("the pour is filled")
        .pieces
        .iter()
        .any(|piece| piece.contains_point(at))
}

fn pieces(board: &PcbBoard) -> usize {
    zone_fills(board).of(0).expect("filled").pieces.len()
}

#[test]
fn thermal_relief_off_joins_the_pad_solidly() {
    let b = board(false, 0.0, pad(PadThermal::RULE));
    assert!(copper_at(&b, 0.75, 0.4), "no gap round the pad");
    assert!(copper_at(&b, 0.0, 0.0));
    assert_eq!(pieces(&b), 1);
}

/// The pad sits in a 0.5 mm gap crossed by four 0.3 mm spokes along its
/// axes, all one piece with the fill.
#[test]
fn thermal_relief_on_crosses_the_air_gap_with_four_spokes() {
    let b = board(true, 0.0, pad(PadThermal::RULE));
    for (x, y) in [(0.75, 0.0), (-0.75, 0.0), (0.0, 0.75), (0.0, -0.75)] {
        assert!(copper_at(&b, x, y), "a spoke at ({x}, {y})");
    }
    for (x, y) in [(0.75, 0.4), (-0.4, 0.75), (0.75, -0.75)] {
        assert!(!copper_at(&b, x, y), "the gap at ({x}, {y})");
    }
    assert!(copper_at(&b, 0.0, 0.0), "the spokes meet over the pad");
    assert!(copper_at(&b, 1.5, 1.5), "the fill beyond the gap");
    assert_eq!(pieces(&b), 1, "the spokes join the pad to the fill");
}

/// A pad's own air gap and spoke count win over the pour's; its spoke width
/// follows the pour.
#[test]
fn a_pads_own_gap_and_spoke_count_win_over_the_pours() {
    let own = PadThermal {
        air_gap: Some(1.0),
        spoke_width: None,
        spokes: Some(2),
    };
    let b = board(true, 0.0, pad(own));
    assert!(copper_at(&b, 1.25, 0.0) && copper_at(&b, -1.25, 0.0));
    assert!(!copper_at(&b, 0.0, 1.25), "no spoke along y");
    assert!(!copper_at(&b, 1.25, 0.6), "the wider gap");
    assert!(copper_at(&b, 0.0, 1.8), "the fill past the 1 mm gap");
    assert!(!copper_at(&b, 1.25, 0.2), "the pour's 0.3 mm spoke width");
}

/// The spokes turn with the pad: a footprint turned 45° has them on the
/// diagonals.
#[test]
fn the_spokes_turn_with_the_pad() {
    let b = board(true, 45.0, pad(PadThermal::RULE));
    assert!(copper_at(&b, 0.53, 0.53), "a spoke on the diagonal");
    assert!(!copper_at(&b, 0.75, 0.0), "the gap on the board's x axis");
}

/// A pad of another net keeps its clearance; relief changes only the pads
/// of the pour's net.
#[test]
fn a_pad_of_another_net_keeps_its_clearance_and_gets_no_spokes() {
    let mut other = pad(PadThermal::RULE);
    other.net = Some(PadNet {
        number: 2,
        name: "VCC".into(),
    });
    let b = board(true, 0.0, other);
    for (x, y) in [(0.75, 0.0), (0.0, 0.75), (0.0, 0.0)] {
        assert!(!copper_at(&b, x, y), "({x}, {y})");
    }
}

/// Caner (2026-11-27): a board's stored thermal relief is honoured as it
/// loads, so a board saved or imported with it on fills with spokes.
#[test]
fn a_board_stored_with_thermal_relief_on_loads_with_its_spokes() {
    let saved = signex_types::format::SnxPcb::new(board(true, 0.0, pad(PadThermal::RULE)))
        .write_string()
        .expect("the board writes");
    let loaded = signex_types::format::SnxPcb::parse(&saved)
        .expect("the board parses")
        .board;
    assert!(loaded.zones[0].thermal_relief, "the switch is kept");
    assert!(copper_at(&loaded, 0.75, 0.0), "a spoke");
    assert!(!copper_at(&loaded, 0.75, 0.4), "the gap");
    assert!(
        crate::ratsnest(&loaded).is_empty(),
        "the spokes join the pad"
    );
}

/// Caner (2026-11-27): a pad whose every spoke is blocked sits alone in its
/// air gap, so the ratsnest (and the DRC's unconnected check, which reads
/// it) draws a line to it; with thermal relief off the pad is joined.
#[test]
fn a_pad_whose_every_spoke_is_blocked_is_left_unconnected() {
    let one_spoke = PadThermal {
        air_gap: None,
        spoke_width: None,
        spokes: Some(1),
    };
    // A net-2 track across the one spoke (along +x), its clearance band
    // over x 0.6..1.8: the spoke meets no fill.
    let blocker = Segment {
        uuid: Uuid::from_u128(20),
        start: Point::new(1.2, -3.0),
        end: Point::new(1.2, 3.0),
        width: 0.2,
        layer: "TopCopper".into(),
        net: 2,
        locked: false,
        attributes: Default::default(),
        unknown_columns: Default::default(),
    };
    let mut joined = pad(PadThermal::RULE);
    joined.uuid = Uuid::from_u128(11);
    joined.number = "2".into();
    joined.position = Point::new(5.0, 5.0);
    let with = |relief: bool| {
        let mut b = board(relief, 0.0, pad(one_spoke));
        b.footprints[0].pads.push(joined.clone());
        b.segments.push(blocker.clone());
        b
    };
    let blocked = with(true);
    assert!(!copper_at(&blocked, 0.75, 0.0), "the spoke is left out");
    let lines = crate::ratsnest(&blocked);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].net, 1);
    assert!(crate::ratsnest(&with(false)).is_empty(), "solid join");
}
