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
        rotation: 0.0,
        net_locked: false,
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
        schematic_link: None,
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
            pad(2, 3.0, 0.0, 2, "TopCopper"),
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
            pad(
                i + 1,
                (n * 7.0) % 13.0 * 3.0,
                (n * 5.0) % 11.0 * 3.0,
                1,
                "TopCopper",
            )
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
fn a_big_net_of_single_pad_islands_is_joined_exactly() {
    // 300 pads, each its own island.
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

#[test]
fn a_stray_pad_joins_the_nearest_pad_of_a_big_routed_island() {
    // 300 pads chained by tracks into one island, plus a stray pad past the
    // last one: the line ends at the last pad, not at the island's centroid.
    let mut pads: Vec<Pad> = (0..300)
        .map(|i| pad(i + 1, i as f64 * 3.0, 0.0, 1, "TopCopper"))
        .collect();
    pads.push(pad(1000, 907.0, 0.0, 1, "TopCopper"));
    let tracks: Vec<Segment> = (0..299)
        .map(|i| {
            let x = i as f64 * 3.0;
            track(i + 1, (x, 0.0), (x + 3.0, 0.0), 1, "TopCopper")
        })
        .collect();
    let lines = ratsnest(&board(
        vec![footprint(1, "TopCopper", pads)],
        tracks,
        vec![],
    ));
    assert_eq!(lines, vec![line(1, at(897, 0), at(907, 0))]);
}

#[test]
fn past_the_pair_limit_islands_fall_back_to_one_pad_each() {
    let island = |x0: i64| -> Vec<PointNm> { (0..2100).map(|i| at(x0 + i, 0)).collect() };
    let groups = [island(0), island(10_000)];
    assert!(cross_island_pairs(&groups) > EXACT_PAIR_LIMIT);
    assert_eq!(join_islands(&groups).len(), 1);
}

#[test]
fn a_track_crossing_a_pad_joins_it_without_an_end_or_the_centre_on_it() {
    // The track runs along y = 0 past the pad at (5, 0.3): its copper
    // (y -0.2..0.8) is crossed, its centre is 0.3 mm from the track.
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 5.0, 0.3, 1, "TopCopper"),
            pad(3, 12.0, 0.0, 1, "TopCopper"),
        ],
    );
    let wire = track(1, (0.0, 0.0), (8.0, 0.0), 1, "TopCopper");
    let lines = ratsnest(&board(vec![fp], vec![wire], vec![]));
    // Pads 1 and 2 are one island; only the gap to pad 3 is left.
    assert_eq!(
        lines,
        vec![line(1, PointNm::new(5 * MM, 300_000), at(12, 0))]
    );
}

#[test]
fn a_track_passing_beside_a_pad_does_not_join_it() {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 5.0, 1.0, 1, "TopCopper"),
        ],
    );
    // Pad 2 spans y 0.5..1.5; the track (half width 0.125) tops out at 0.125.
    let wire = track(1, (0.0, 0.0), (8.0, 0.0), 1, "TopCopper");
    assert_eq!(ratsnest(&board(vec![fp], vec![wire], vec![])).len(), 1);
}

#[test]
fn overlapping_same_net_pads_join_without_a_centre_inside_the_other() {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 0.8, 0.0, 1, "TopCopper"),
        ],
    );
    assert!(ratsnest(&board(vec![fp], vec![], vec![])).is_empty());
}

#[test]
fn overlapping_pads_of_another_net_or_layer_stay_apart() {
    let other_net = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 0.8, 0.0, 2, "TopCopper"),
            pad(3, 5.0, 0.0, 2, "TopCopper"),
        ],
    );
    assert_eq!(ratsnest(&board(vec![other_net], vec![], vec![])).len(), 1);
    let other_layer = footprint(
        2,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 0.8, 0.0, 1, "BottomCopper"),
        ],
    );
    assert_eq!(ratsnest(&board(vec![other_layer], vec![], vec![])).len(), 1);
}

#[test]
fn pads_that_stop_short_of_touching_stay_apart() {
    // 1 mm squares 1.2 mm apart leave a 0.2 mm gap.
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 1.2, 0.0, 1, "TopCopper"),
        ],
    );
    assert_eq!(ratsnest(&board(vec![fp], vec![], vec![])).len(), 1);
}

// ---- crossing tracks and zones (#2200) ------------------------------------

fn zone(net: u32, layer: &str, rect: (f64, f64, f64, f64)) -> signex_types::pcb::Zone {
    let (x0, y0, x1, y1) = rect;
    signex_types::pcb::Zone {
        uuid: id(4000 + u128::from(net)),
        net,
        net_name: format!("N{net}"),
        layer: layer.into(),
        outline: vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    }
}

/// Two 1 mm pads, (0,0) and (20,0), net 1, plus the extra copper.
fn two_pads_with(
    segments: Vec<Segment>,
    vias: Vec<Via>,
    zones: Vec<signex_types::pcb::Zone>,
) -> PcbBoard {
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 20.0, 0.0, 1, "TopCopper"),
        ],
    );
    let mut b = board(vec![fp], segments, vias);
    b.zones = zones;
    b
}

fn apart() -> Vec<RatLine> {
    vec![line(1, at(0, 0), at(20, 0))]
}

#[test]
fn tracks_that_cross_mid_segment_are_one_island() {
    // 0,0 -> 10,10 and 20,0 -> 10,10 would meet at an end; instead cross:
    // A: (0,0)-(12,12) from pad 1; B: (20,0)-(8,12) from pad 2. They cross
    // at (10,10)... neither end lies on the other.
    let b = two_pads_with(
        vec![
            track(1, (0.0, 0.0), (12.0, 12.0), 1, "TopCopper"),
            track(2, (20.0, 0.0), (8.0, 12.0), 1, "TopCopper"),
        ],
        vec![],
        vec![],
    );
    assert!(ratsnest(&b).is_empty());
}

#[test]
fn parallel_tracks_with_a_one_nm_gap_do_not_join() {
    // Width 0.25: edges are 0.125 mm out. Centres 0.25 mm + 1 nm apart
    // leave a 1 nm gap; 0.25 mm apart exactly touch.
    let gap = |dy: f64| {
        two_pads_with(
            vec![
                track(1, (0.0, 5.0), (10.0, 5.0), 1, "TopCopper"),
                track(2, (0.0, 5.0 + dy), (10.0, 5.0 + dy), 1, "TopCopper"),
                // Tie each pad to its own track so the pads' islands are
                // decided by whether the two tracks join.
                track(3, (0.0, 0.0), (0.0, 5.0), 1, "TopCopper"),
                track(4, (20.0, 0.0), (20.0, 5.0 + dy), 1, "TopCopper"),
                track(5, (10.0, 5.0 + dy), (20.0, 5.0 + dy), 1, "TopCopper"),
            ],
            vec![],
            vec![],
        )
    };
    assert!(ratsnest(&gap(0.25)).is_empty());
    assert_eq!(ratsnest(&gap(0.250_001)), apart());
}

#[test]
fn crossing_tracks_on_other_layers_or_nets_do_not_join() {
    let crossing = |layer_b: &str, net_b: u32| {
        two_pads_with(
            vec![
                track(1, (0.0, 0.0), (12.0, 12.0), 1, "TopCopper"),
                track(2, (20.0, 0.0), (8.0, 12.0), net_b, layer_b),
            ],
            vec![],
            vec![],
        )
    };
    assert_eq!(ratsnest(&crossing("BottomCopper", 1)), apart());
    assert_eq!(ratsnest(&crossing("TopCopper", 2)), apart());
}

#[test]
fn a_zone_joins_the_pads_it_covers() {
    // Pad 1 at 0,0 and pad 2 at 20,0 both sit in one zone.
    let b = two_pads_with(
        vec![],
        vec![],
        vec![zone(1, "TopCopper", (-2.0, -2.0, 22.0, 2.0))],
    );
    assert!(ratsnest(&b).is_empty());
}

#[test]
fn a_zone_edge_one_nm_short_of_a_pad_does_not_join_it() {
    // Pad 2 spans x 19.5..20.5. The zone ends at 19.5 (touching) or
    // 19.499999 (1 nm short).
    let end = |x1: f64| {
        two_pads_with(
            vec![],
            vec![],
            vec![zone(1, "TopCopper", (-2.0, -2.0, x1, 2.0))],
        )
    };
    assert!(ratsnest(&end(19.5)).is_empty());
    assert_eq!(ratsnest(&end(19.499_999)), apart());
}

#[test]
fn a_zone_on_another_layer_or_net_joins_nothing() {
    let r = (-2.0, -2.0, 22.0, 2.0);
    assert_eq!(
        ratsnest(&two_pads_with(
            vec![],
            vec![],
            vec![zone(1, "BottomCopper", r)]
        )),
        apart()
    );
    assert_eq!(
        ratsnest(&two_pads_with(
            vec![],
            vec![],
            vec![zone(2, "TopCopper", r)]
        )),
        apart()
    );
}

#[test]
fn a_track_or_a_via_on_a_zone_joins_through_it() {
    // Pad 1 -> track into the zone; pad 2 -> track into the zone. The
    // zone sits between, touching neither pad.
    let z = zone(1, "TopCopper", (8.0, -2.0, 12.0, 2.0));
    let tracks = vec![
        track(1, (0.0, 0.0), (9.0, 0.0), 1, "TopCopper"),
        track(2, (20.0, 0.0), (11.0, 0.0), 1, "TopCopper"),
    ];
    assert!(ratsnest(&two_pads_with(tracks.clone(), vec![], vec![z.clone()])).is_empty());
    // Without the zone the two tracks leave a gap.
    assert_eq!(
        ratsnest(&two_pads_with(tracks, vec![], vec![])),
        vec![line(1, at(0, 0), at(20, 0))]
    );
}

#[test]
fn a_via_joins_a_zone_on_a_layer_it_spans_but_not_one_it_does_not() {
    // Pad 1 (top) -> track -> via at 1,0 (through); pad 2 is on the
    // bottom and sits on a bottom zone that the via lands in.
    let top = footprint(1, "TopCopper", vec![pad(1, 0.0, 0.0, 1, "TopCopper")]);
    let bottom = footprint(2, "BottomCopper", vec![pad(2, 0.0, 8.0, 1, "BottomCopper")]);
    let mut b = board(
        vec![top, bottom],
        vec![track(1, (0.0, 0.0), (1.0, 0.0), 1, "TopCopper")],
        vec![via(1, (1.0, 0.0), 1)],
    );
    b.zones = vec![zone(1, "BottomCopper", (-2.0, -2.0, 2.0, 10.0))];
    assert!(ratsnest(&b).is_empty());
    // A blind via spanning only the top layer does not reach the bottom zone.
    b.vias[0].via_type = ViaType::Blind;
    b.vias[0].layers = vec!["TopCopper".into()];
    assert_eq!(ratsnest(&b), vec![line(1, at(0, 0), at(0, 8))]);
}

#[test]
fn a_concave_zone_joins_only_where_it_has_copper() {
    // A U: arms x -2..2 and x 18..22 up from a base y 4..6; pads at y=0 sit
    // in the arms; the notch (x 2..18, y -2..4) is empty.
    let mut z = zone(1, "TopCopper", (0.0, 0.0, 0.0, 0.0));
    let pts = [
        (-2.0, -2.0),
        (2.0, -2.0),
        (2.0, 4.0),
        (18.0, 4.0),
        (18.0, -2.0),
        (22.0, -2.0),
        (22.0, 6.0),
        (-2.0, 6.0),
    ];
    z.outline = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
    assert!(ratsnest(&two_pads_with(vec![], vec![], vec![z.clone()])).is_empty());
    // A pad in the notch is not on the zone.
    let fp = footprint(
        1,
        "TopCopper",
        vec![
            pad(1, 0.0, 0.0, 1, "TopCopper"),
            pad(2, 10.0, 0.0, 1, "TopCopper"),
        ],
    );
    let mut b = board(vec![fp], vec![], vec![]);
    b.zones = vec![z];
    assert_eq!(ratsnest(&b), vec![line(1, at(0, 0), at(10, 0))]);
}

/// Zone A over pad 1 (x -2..10) and zone B over pad 2 (x `b_x0`..22).
fn zones_a_and_b(b_x0: f64, layer_b: &str, net_b: u32) -> PcbBoard {
    let a = zone(1, "TopCopper", (-2.0, -2.0, 10.0, 2.0));
    let mut b = zone(net_b, layer_b, (b_x0, -2.0, 22.0, 2.0));
    b.uuid = id(4900);
    two_pads_with(vec![], vec![], vec![a, b])
}

#[test]
fn overlapping_same_net_zones_join_their_pads() {
    assert!(ratsnest(&zones_a_and_b(8.0, "TopCopper", 1)).is_empty());
}

#[test]
fn zones_touching_join_and_zones_1_nm_apart_do_not() {
    assert!(ratsnest(&zones_a_and_b(10.0, "TopCopper", 1)).is_empty());
    assert_eq!(
        ratsnest(&zones_a_and_b(10.000_001, "TopCopper", 1)),
        apart()
    );
}

#[test]
fn overlapping_zones_on_another_layer_or_net_do_not_join() {
    // Pad 2 is a top pad, so a bottom zone B also leaves it unjoined.
    assert_eq!(ratsnest(&zones_a_and_b(8.0, "BottomCopper", 1)), apart());
    assert_eq!(ratsnest(&zones_a_and_b(8.0, "TopCopper", 2)), apart());
}

// ---- degenerate zones: no area, so a segment or a point, never the plane --

fn flat_zone(uuid: u128, pts: &[(f64, f64)]) -> signex_types::pcb::Zone {
    let mut z = zone(1, "TopCopper", (0.0, 0.0, 0.0, 0.0));
    z.uuid = id(uuid);
    z.outline = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
    z
}

#[test]
fn a_zone_with_no_area_joins_nothing_it_does_not_touch() {
    // Pads at (0,0) and (20,0). Each flat zone sits 8 mm from pad 1 and
    // 8 mm from pad 2, or (the single point) 5 mm above pad 1.
    let lone = |pts: &[(f64, f64)]| two_pads_with(vec![], vec![], vec![flat_zone(4800, pts)]);
    assert_eq!(
        ratsnest(&lone(&[(8.0, 0.0), (10.0, 0.0), (12.0, 0.0)])),
        apart()
    );
    assert_eq!(ratsnest(&lone(&[(8.0, 0.0), (12.0, 0.0)])), apart());
    assert_eq!(ratsnest(&lone(&[(10.0, 0.0)])), apart());
    assert_eq!(
        ratsnest(&lone(&[(0.0, 5.0), (0.0, 5.0), (0.0, 5.0)])),
        apart()
    );
    // Collinear but slanted, off the pads.
    assert_eq!(
        ratsnest(&lone(&[(5.0, 5.0), (7.0, 7.0), (9.0, 9.0)])),
        apart()
    );
}

#[test]
fn a_zone_with_no_area_joins_what_its_segment_crosses() {
    let lone = |pts: &[(f64, f64)]| two_pads_with(vec![], vec![], vec![flat_zone(4800, pts)]);
    // The segment runs through both pads: joined, with 3 points or 2.
    assert!(ratsnest(&lone(&[(-2.0, 0.0), (10.0, 0.0), (22.0, 0.0)])).is_empty());
    assert!(ratsnest(&lone(&[(-2.0, 0.0), (22.0, 0.0)])).is_empty());
    // Ends exactly on pad 2's edge (19.5) joins; 1 nm short does not.
    assert!(ratsnest(&lone(&[(-2.0, 0.0), (19.5, 0.0)])).is_empty());
    assert_eq!(ratsnest(&lone(&[(-2.0, 0.0), (19.499_999, 0.0)])), apart());
    // A single point on pad 2 touches only pad 2.
    assert_eq!(ratsnest(&lone(&[(20.0, 0.0)])), apart());
}

#[test]
fn a_zone_with_no_area_beside_a_real_zone_meets_it_only_by_touching() {
    // Zone A over pad 1 is flat (x -2..10 on y=0); B is a real zone over
    // pad 2 starting at b_x0.
    let flat_a = |b_x0: f64| {
        let mut b = zone(1, "TopCopper", (b_x0, -2.0, 22.0, 2.0));
        b.uuid = id(4900);
        two_pads_with(
            vec![],
            vec![],
            vec![flat_zone(4800, &[(-2.0, 0.0), (4.0, 0.0), (10.0, 0.0)]), b],
        )
    };
    assert!(ratsnest(&flat_a(8.0)).is_empty());
    assert!(ratsnest(&flat_a(10.0)).is_empty());
    assert_eq!(ratsnest(&flat_a(10.000_001)), apart());
    // Two flat zones: crossing joins, parallel 1 nm apart does not.
    let two_flat = |y: f64| {
        two_pads_with(
            vec![],
            vec![],
            vec![
                flat_zone(4800, &[(-2.0, 0.0), (10.0, 0.0)]),
                flat_zone(4801, &[(10.0, y), (22.0, y)]),
            ],
        )
    };
    assert!(ratsnest(&two_flat(0.0)).is_empty());
    assert_eq!(ratsnest(&two_flat(0.000_001)), apart());
}
