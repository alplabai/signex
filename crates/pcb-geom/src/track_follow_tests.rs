use super::*;
use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pad(n: u128, x: f64, net: u32) -> Pad {
    Pad {
        uuid: id(n),
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

/// U1 at (10, 0) with pad 1 at local x = -1 (net 1) and pad 2 at x = +1
/// (net 2).
fn footprint() -> Footprint {
    Footprint {
        uuid: id(100),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad(1, -1.0, 1), pad(2, 1.0, 2)],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32, layer: &str) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: id(200 + n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: layer.into(),
        net,
        unknown_columns: Default::default(),
    }
}

fn board(segments: Vec<Segment>) -> PcbBoard {
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
        footprints: vec![footprint()],
        segments,
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

fn carried() -> BTreeSet<Uuid> {
    [id(100)].into()
}

#[test]
fn only_the_end_on_a_same_net_pad_of_a_shared_layer_follows() {
    let board = board(vec![
        // Ends on pad 1 (9, 0), runs off to (0, 0).
        track(1, (0.0, 0.0), (9.2, 0.1), 1, "TopCopper"),
        // Starts on pad 2 (11, 0).
        track(2, (11.0, 0.0), (20.0, 0.0), 2, "TopCopper"),
        // On pad 1 but another net.
        track(3, (9.0, 0.0), (0.0, 5.0), 3, "TopCopper"),
        // On pad 1 but on the bottom copper the pad does not reach.
        track(4, (9.0, 0.0), (0.0, 6.0), 1, "BottomCopper"),
        // Nowhere near a pad.
        track(5, (0.0, 9.0), (5.0, 9.0), 1, "TopCopper"),
    ]);
    let ends = tracks_on_pads(&board, &carried());
    assert_eq!(
        ends,
        [
            TrackEnds {
                track: id(201),
                index: 0,
                start: false,
                end: true,
            },
            TrackEnds {
                track: id(202),
                index: 1,
                start: true,
                end: false,
            },
        ]
    );
}

#[test]
fn a_footprint_not_moved_takes_nothing_along() {
    let board = board(vec![track(1, (0.0, 0.0), (9.0, 0.0), 1, "TopCopper")]);
    assert!(tracks_on_pads(&board, &[id(999)].into()).is_empty());
}

fn through_via(n: u128, x: f64, net: u32) -> signex_types::pcb::Via {
    signex_types::pcb::Via {
        attributes: Default::default(),
        uuid: id(n),
        position: Point::new(x, 0.0),
        diameter: 0.8,
        drill: 0.4,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net,
        locked: false,
        layer_sizes: None,
        via_type: signex_types::pcb::ViaType::Through,
        unknown_columns: Default::default(),
    }
}

/// A stub from the via centre to a same-net pad 0.3 mm away has both ends on
/// the via's copper; only the end at its centre follows.
#[test]
fn a_stub_to_a_near_pad_keeps_its_pad_end_when_the_via_moves() {
    let mut board = board(vec![track(1, (8.7, 0.0), (9.0, 0.0), 1, "TopCopper")]);
    board.vias.push(through_via(300, 8.7, 1));
    let ends = tracks_on_vias(&board, &[id(300)].into());
    assert_eq!(
        ends,
        [TrackEnds {
            track: id(201),
            index: 0,
            start: true,
            end: false,
        }]
    );
    let moved = ends[0].follow(&board.segments[0], Point::new(-2.0, 1.0));
    assert_eq!(moved.end, Point::new(9.0, 0.0), "the pad end stays");
}

fn arc(n: u128, start: (f64, f64), end: (f64, f64), net: u32, layer: &str) -> TrackArc {
    let (cx, cy) = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
    let half = (end.0 - start.0) / 2.0;
    TrackArc {
        uuid: id(400 + n),
        start: Point::new(start.0, start.1),
        mid: Point::new(cx, cy + half),
        end: Point::new(end.0, end.1),
        width: 0.25,
        layer: layer.into(),
        net,
    }
}

#[test]
fn only_a_copper_arc_end_at_the_via_on_its_net_and_layer_follows() {
    let mut board = board(Vec::new());
    board.vias.push(through_via(300, 2.0, 1));
    board.free.arcs = vec![
        arc(1, (2.0, 0.0), (6.0, 0.0), 1, "TopCopper"),
        arc(2, (2.0, 0.0), (6.0, 0.0), 2, "TopCopper"),
        arc(3, (2.0, 0.0), (6.0, 0.0), 1, "TopSilkscreen"),
        arc(4, (2.1, 0.0), (6.0, 0.0), 1, "TopCopper"),
    ];
    assert_eq!(
        arcs_on_vias(&board, &[id(300)].into()),
        [ArcEnds {
            arc: id(401),
            index: 0,
            start: true,
            end: false,
        }]
    );
}

#[test]
fn an_arc_following_one_end_keeps_its_other_end_and_its_sweep() {
    let before = arc(1, (2.0, 0.0), (6.0, 0.0), 1, "TopCopper");
    let ends = ArcEnds {
        arc: before.uuid,
        index: 0,
        start: true,
        end: false,
    };
    let after = ends.follow(&before, Point::new(-1.0, 3.0));
    assert_eq!(after.start, Point::new(1.0, 3.0));
    assert_eq!(after.end, before.end);
    let (old, new) = (
        before.sweep().unwrap(),
        after.sweep().expect("still an arc"),
    );
    assert!((old.sweep - new.sweep).abs() < 1e-9, "{old:?} {new:?}");

    let both = ArcEnds { end: true, ..ends };
    let shifted = both.follow(&before, Point::new(-1.0, 3.0));
    assert_eq!(shifted.mid, Point::new(3.0, 5.0));
    assert_eq!(shifted.end, Point::new(5.0, 3.0));
}

#[test]
fn follow_shifts_only_the_named_ends() {
    let segment = track(1, (0.0, 0.0), (9.0, 0.0), 1, "TopCopper");
    let ends = TrackEnds {
        track: segment.uuid,
        index: 0,
        start: false,
        end: true,
    };
    let moved = ends.follow(&segment, Point::new(2.0, -1.0));
    assert_eq!(moved.start, Point::new(0.0, 0.0));
    assert_eq!(moved.end, Point::new(11.0, -1.0));
    let both = TrackEnds {
        start: true,
        ..ends
    };
    let moved = both.follow(&segment, Point::new(2.0, -1.0));
    assert_eq!(moved.start, Point::new(2.0, -1.0));
    assert_eq!(moved.end, Point::new(11.0, -1.0));
}
