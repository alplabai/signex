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
