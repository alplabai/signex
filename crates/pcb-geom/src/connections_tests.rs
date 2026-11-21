//! #2460: the status bar's Routed and Unrouted counts.

use super::*;
use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType, Point, Segment, Zone};
use signex_types::zone_pour::POUR_FILL;

fn pad(n: u128, x: f64, net: u32) -> Pad {
    Pad {
        uuid: Uuid::from_u128(n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: net,
            name: format!("N{net}"),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn part(reference: &str, n: u128, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid: Uuid::from_u128(1000 + n),
        reference: reference.into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn track(n: u128, from: f64, to: f64, net: u32) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(2000 + n),
        start: Point::new(from, 0.0),
        end: Point::new(to, 0.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// A net-1 pour from x = -5 to 25 on the top copper.
fn pour() -> Zone {
    Zone {
        uuid: Uuid::from_u128(4000),
        net: 1,
        net_name: "N1".into(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(-5.0, -5.0),
            Point::new(25.0, -5.0),
            Point::new(25.0, 5.0),
            Point::new(-5.0, 5.0),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.2,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unknown: Default::default(),
    }
}

fn board(footprints: Vec<Footprint>, segments: Vec<Segment>, zones: Vec<Zone>) -> PcbBoard {
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
        footprints,
        segments,
        vias: Vec::new(),
        zones,
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

fn counts(board: &PcbBoard) -> ConnectionCounts {
    connection_counts(board, &crate::ratsnest(board))
}

fn name(designator: &str, pad: &str) -> PadName {
    PadName {
        designator: designator.into(),
        pad: pad.into(),
    }
}

/// C1-1 at x = 0, U1-2 at 10 and J1-3 at 20, all net 1.
fn three_pads() -> Vec<Footprint> {
    vec![
        part("C1", 1, vec![pad(1, 0.0, 1)]),
        part("U1", 2, vec![pad(2, 10.0, 1)]),
        part("J1", 3, vec![pad(3, 20.0, 1)]),
    ]
}

#[test]
fn an_empty_board_needs_no_connection() {
    assert_eq!(
        counts(&board(Vec::new(), Vec::new(), Vec::new())),
        ConnectionCounts::default()
    );
    let lone = vec![part("R1", 1, vec![pad(1, 0.0, 1)])];
    let c = counts(&board(lone, Vec::new(), Vec::new()));
    assert_eq!((c.total, c.unrouted, c.routed()), (0, 0, 0));
}

#[test]
fn a_fully_routed_net_counts_as_routed() {
    let tracks = vec![track(1, 0.0, 10.0, 1), track(2, 10.0, 20.0, 1)];
    let c = counts(&board(three_pads(), tracks, Vec::new()));
    assert_eq!((c.total, c.unrouted, c.routed()), (2, 0, 2));
    assert!(c.per_net.is_empty());
}

#[test]
fn a_partly_routed_net_names_the_pads_still_to_join() {
    let c = counts(&board(
        three_pads(),
        vec![track(1, 0.0, 10.0, 1)],
        Vec::new(),
    ));
    assert_eq!((c.total, c.unrouted, c.routed()), (2, 1, 1));
    let net = &c.per_net[&1];
    assert_eq!(net.lines.len(), 1);
    assert_eq!(net.pads, vec![(name("U1", "2"), name("J1", "3"))]);
    assert!(!net.pour_will_connect);
}

#[test]
fn nothing_routed_needs_one_less_than_the_pads() {
    let c = counts(&board(three_pads(), Vec::new(), Vec::new()));
    assert_eq!((c.total, c.unrouted, c.routed()), (2, 2, 0));
}

/// Two pads whose copper overlaps are one island: they need no connection.
#[test]
fn overlapping_pads_need_no_connection() {
    let both = vec![part("U1", 1, vec![pad(1, 0.0, 1), pad(2, 0.5, 1)])];
    let c = counts(&board(both, Vec::new(), Vec::new()));
    assert_eq!((c.total, c.unrouted), (0, 0));
}

#[test]
fn a_pour_over_the_pads_routes_them() {
    let c = counts(&board(three_pads(), Vec::new(), vec![pour()]));
    assert_eq!((c.total, c.unrouted, c.routed()), (2, 0, 2));
}

/// A pad the pour does not reach yet is unrouted, with the note that a
/// same-net pour will make the connection.
#[test]
fn a_pad_outside_the_pour_is_unrouted_and_the_pour_will_connect_it() {
    let mut zone = pour();
    zone.holes = vec![vec![
        Point::new(18.0, -2.0),
        Point::new(22.0, -2.0),
        Point::new(22.0, 2.0),
        Point::new(18.0, 2.0),
    ]];
    let c = counts(&board(three_pads(), Vec::new(), vec![zone]));
    assert_eq!((c.total, c.unrouted, c.routed()), (2, 1, 1));
    let net = &c.per_net[&1];
    assert!(net.pour_will_connect);
    assert_eq!(net.pads, vec![(name("U1", "2"), name("J1", "3"))]);
}

#[test]
fn routed_never_exceeds_total() {
    let c = ConnectionCounts {
        total: 1,
        unrouted: 3,
        per_net: BTreeMap::new(),
    };
    assert_eq!(c.routed(), 0);
}
