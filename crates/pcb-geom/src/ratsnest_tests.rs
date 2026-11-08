use super::*;
use signex_types::pcb::{PadNet, PadShape, Point, Segment, Via};
use uuid::Uuid;

const MM: i64 = 1_000_000;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pad(n: u128, x: f64, y: f64, net: u32, layer: &str) -> Pad {
    Pad {
        uuid: id(n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, y),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec![layer.into()],
        net: Some(PadNet {
            number: net,
            name: format!("N{net}"),
        }),
        roundrect_ratio: 0.0,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(n: u128, layer: &str, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid: id(1000 + n),
        reference: format!("U{n}"),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: layer.into(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32, layer: &str) -> Segment {
    Segment {
        uuid: id(2000 + n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: layer.into(),
        net,
        unknown_columns: Default::default(),
    }
}

fn via(n: u128, at: (f64, f64), net: u32) -> Via {
    Via {
        uuid: id(3000 + n),
        position: Point::new(at.0, at.1),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    }
}

fn board(footprints: Vec<Footprint>, segments: Vec<Segment>, vias: Vec<Via>) -> PcbBoard {
    PcbBoard {
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
        vias,
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn at(x: i64, y: i64) -> PointNm {
    PointNm::new(x * MM, y * MM)
}

fn line(net: u32, a: PointNm, b: PointNm) -> RatLine {
    RatLine {
        net,
        from: a.min(b),
        to: a.max(b),
    }
}

#[test]
fn an_empty_board_and_a_lone_pad_have_no_ratsnest() {
    assert!(ratsnest(&board(vec![], vec![], vec![])).is_empty());
    let lone = footprint(1, "TopCopper", vec![pad(1, 0.0, 0.0, 1, "TopCopper")]);
    assert!(ratsnest(&board(vec![lone], vec![], vec![])).is_empty());
}

#[test]
fn unrouted_pads_get_the_shortest_tree() {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 10.0, 0.0, 1, "TopCopper"),
            pad(3, 3.0, 0.0, 1, "TopCopper"),
        ],
    );
    let lines = ratsnest(&board(vec![fp], vec![], vec![]));
    assert_eq!(
        lines,
        vec![line(1, at(0, 0), at(3, 0)), line(1, at(3, 0), at(10, 0)),]
    );
}

#[test]
fn a_routed_pair_is_one_island() {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 5.0, 0.0, 1, "TopCopper"),
        ],
    );
    let wire = track(1, (0.0, 0.0), (5.0, 0.0), 1, "TopCopper");
    assert!(ratsnest(&board(vec![fp], vec![wire], vec![])).is_empty());
}

#[test]
fn a_partly_routed_net_joins_the_closest_pads_of_the_islands() {
    // Pads at x = 0, 5 (routed together) and 8; the 5 -> 8 gap is the link.
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 5.0, 0.0, 1, "TopCopper"),
            pad(3, 8.0, 0.0, 1, "TopCopper"),
        ],
    );
    let wire = track(1, (0.0, 0.0), (5.0, 0.0), 1, "TopCopper");
    let lines = ratsnest(&board(vec![fp], vec![wire], vec![]));
    assert_eq!(lines, vec![line(1, at(5, 0), at(8, 0))]);
}

#[test]
fn a_via_carries_a_net_between_layers() {
    let top = footprint(1, "TopCopper", vec![pad(1, 0.0, 0.0, 1, "TopCopper")]);
    let bottom = footprint(
        2,
        "BottomCopper",
        vec![pad(2, 0.0, 10.0, 1, "BottomCopper")],
    );
    let segments = vec![
        track(1, (0.0, 0.0), (0.0, 4.0), 1, "TopCopper"),
        track(2, (0.0, 4.0), (0.0, 10.0), 1, "BottomCopper"),
    ];
    let with_via = board(
        vec![top.clone(), bottom.clone()],
        segments.clone(),
        vec![via(1, (0.0, 4.0), 1)],
    );
    assert!(ratsnest(&with_via).is_empty());
    // Without the via the two layers are apart.
    let without = board(vec![top, bottom], segments, vec![]);
    assert_eq!(ratsnest(&without).len(), 1);
}

#[test]
fn a_track_on_another_layer_does_not_join_a_surface_pad() {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 5.0, 0.0, 1, "TopCopper"),
        ],
    );
    let wire = track(1, (0.0, 0.0), (5.0, 0.0), 1, "BottomCopper");
    let lines = ratsnest(&board(vec![fp], vec![wire], vec![]));
    assert_eq!(lines, vec![line(1, at(0, 0), at(5, 0))]);
}

#[test]
fn a_through_hole_pad_joins_a_track_on_any_layer() {
    let mut thru = pad(1, 0.0, 0.0, 1, "TopCopper");
    thru.pad_type = PadType::Thru;
    let fp = footprint(
        1,
        "TopCopper",
        vec![thru, pad(2, 5.0, 0.0, 1, "BottomCopper")],
    );
    let wire = track(1, (0.0, 0.0), (5.0, 0.0), 1, "BottomCopper");
    assert!(ratsnest(&board(vec![fp], vec![wire], vec![])).is_empty());
}

#[test]
fn copper_of_another_net_never_joins_and_net_zero_is_no_net() {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 5.0, 0.0, 1, "TopCopper"),
            pad(3, 0.0, 5.0, 0, "TopCopper"),
            pad(4, 5.0, 5.0, 0, "TopCopper"),
        ],
    );
    let other_net = track(1, (0.0, 0.0), (5.0, 0.0), 2, "TopCopper");
    let lines = ratsnest(&board(vec![fp], vec![other_net], vec![]));
    assert_eq!(lines, vec![line(1, at(0, 0), at(5, 0))]);
}

#[test]
fn nets_are_independent_and_sorted_by_net() {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 2, "TopCopper"),
            pad(2, 1.0, 0.0, 2, "TopCopper"),
            pad(3, 0.0, 9.0, 1, "TopCopper"),
            pad(4, 4.0, 9.0, 1, "TopCopper"),
        ],
    );
    let lines = ratsnest(&board(vec![fp], vec![], vec![]));
    let nets: Vec<u32> = lines.iter().map(|l| l.net).collect();
    assert_eq!(nets, vec![1, 2]);
}

#[test]
fn the_result_does_not_depend_on_item_order() {
    let pads: Vec<Pad> = (0..12)
        .map(|i| {
            let n = i as f64;
            pad(i + 1, (n * 7.0) % 13.0, (n * 5.0) % 11.0, 1, "TopCopper")
        })
        .collect();
    let mut reversed = pads.clone();
    reversed.reverse();
    let a = ratsnest(&board(
        vec![footprint(1, "TopCopper", pads)],
        vec![],
        vec![],
    ));
    let b = ratsnest(&board(
        vec![footprint(1, "TopCopper", reversed)],
        vec![],
        vec![],
    ));
    assert_eq!(a.len(), 11);
    assert_eq!(a, b);
}

#[test]
fn a_bottom_side_pad_ends_the_line_at_its_mirrored_centre() {
    let mut fp = footprint(
        1,
        "BottomCopper",
        vec![
            pad(1, 4.0, 0.0, 1, "BottomCopper"),
            pad(2, -4.0, 0.0, 1, "BottomCopper"),
        ],
    );
    fp.position = Point::new(20.0, 0.0);
    let lines = ratsnest(&board(vec![fp], vec![], vec![]));
    // The x mirror puts local +4 at world 16 and local -4 at world 24.
    assert_eq!(lines, vec![line(1, at(16, 0), at(24, 0))]);
}

#[test]
fn a_big_net_uses_one_representative_per_island() {
    // 300 pads, each its own island: past EXACT_PAD_LIMIT.
    let pads: Vec<Pad> = (0..300)
        .map(|i| pad(i + 1, i as f64 * 3.0, 0.0, 1, "TopCopper"))
        .collect();
    let lines = ratsnest(&board(
        vec![footprint(1, "TopCopper", pads)],
        vec![],
        vec![],
    ));
    assert_eq!(lines.len(), 299);
    assert!(lines.iter().all(|l| l.to.x - l.from.x == 3 * MM));
}
