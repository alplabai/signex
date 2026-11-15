use super::*;
use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType, Point, Segment};
use signex_types::zone_keepout::KEEPOUT_FILL;
use signex_types::zone_pour::POUR_FILL;

const MM: i64 = 1_000_000;

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
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn zone(n: u128, net: u32, fill: &str, layer: &str, rect: (f64, f64, f64, f64)) -> Zone {
    let (x0, y0, x1, y1) = rect;
    Zone {
        uuid: Uuid::from_u128(n),
        net,
        net_name: String::new(),
        layer: layer.into(),
        outline: vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: fill.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.5,
        min_thickness: 0.0,
        unknown: Default::default(),
    }
}

/// A net-1 pour over (-10,-10)..(10,10) on the top copper, clearance 0.5,
/// with `pads` on a footprint at the origin.
fn board(pads: Vec<Pad>) -> PcbBoard {
    let footprint = Footprint {
        uuid: Uuid::from_u128(100),
        reference: "U1".into(),
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
    };
    PcbBoard {
        sketch: None,
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
        zones: vec![zone(
            1,
            1,
            POUR_FILL,
            "TopCopper",
            (-10.0, -10.0, 10.0, 10.0),
        )],
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn pieces(board: &PcbBoard) -> Vec<Polygon> {
    zone_fills(board)
        .of(0)
        .expect("the pour is filled")
        .pieces
        .clone()
}

fn twice_area(pieces: &[Polygon]) -> i128 {
    pieces.iter().map(Polygon::twice_area).sum()
}

const FULL: i128 = 2 * (20 * MM as i128) * (20 * MM as i128);

#[test]
fn a_pour_with_nothing_on_it_fills_its_outline() {
    let fill = pieces(&board(Vec::new()));
    assert_eq!(fill.len(), 1);
    assert_eq!(twice_area(&fill), FULL);
}

#[test]
fn copper_of_another_net_is_cut_out_with_the_clearance_and_no_closer() {
    let b = board(vec![pad(1, 0.0, 2)]);
    let fill = pieces(&b);
    assert_eq!(fill.len(), 1);
    assert_eq!(fill[0].holes.len(), 1);
    let pad_copper = pad_shape(&b.footprints[0], &b.footprints[0].pads[0]).unwrap();
    let clearance = MM / 2;
    let (near, far) = (
        pad_copper.inflated(clearance - 1),
        pad_copper.inflated(clearance + MM / 50),
    );
    for p in &fill[0].holes[0] {
        assert!(!near.contains(*p, 0), "{p:?} inside the clearance");
        // ... and not much further: the hole hugs the clearance.
        assert!(far.contains(*p, 0), "{p:?} too far out");
    }
}

#[test]
fn copper_of_the_pours_own_net_is_filled_over_solidly() {
    let fill = pieces(&board(vec![pad(1, 0.0, 1)]));
    assert_eq!(fill.len(), 1);
    assert!(fill[0].holes.is_empty(), "no relief, no cut");
    assert_eq!(twice_area(&fill), FULL);
}

#[test]
fn a_pad_of_no_net_is_cleared_like_any_other_net() {
    let mut no_net = pad(1, 0.0, 0);
    no_net.net = None;
    assert_eq!(pieces(&board(vec![no_net]))[0].holes.len(), 1);
}

#[test]
fn copper_on_another_layer_is_not_cut() {
    let mut b = board(Vec::new());
    b.segments.push(Segment {
        uuid: Uuid::from_u128(7),
        start: Point::new(-20.0, 0.0),
        end: Point::new(20.0, 0.0),
        width: 0.25,
        layer: "BottomCopper".into(),
        net: 2,
        unknown_columns: Default::default(),
    });
    assert_eq!(twice_area(&pieces(&b)), FULL);
    b.segments[0].layer = "TopCopper".into();
    assert_eq!(pieces(&b).len(), 2, "a track across splits the pour");
}

#[test]
fn a_keepout_on_the_layer_and_the_pours_own_holes_are_left_empty() {
    let mut b = board(Vec::new());
    b.zones[0].holes = vec![vec![
        Point::new(-8.0, -8.0),
        Point::new(-6.0, -8.0),
        Point::new(-6.0, -6.0),
        Point::new(-8.0, -6.0),
    ]];
    b.zones
        .push(zone(2, 0, KEEPOUT_FILL, "TopCopper", (6.0, 6.0, 8.0, 8.0)));
    b.zones.push(zone(
        3,
        0,
        KEEPOUT_FILL,
        "BottomCopper",
        (-8.0, 6.0, -6.0, 8.0),
    ));
    let fill = pieces(&b);
    assert_eq!(fill.len(), 1);
    assert_eq!(
        fill[0].holes.len(),
        2,
        "the hole and the top keepout, not the bottom one"
    );
    assert_eq!(
        twice_area(&fill),
        FULL - 2 * 2 * (4 * MM as i128) * (MM as i128)
    );
}

#[test]
fn a_solid_zone_of_another_net_is_cleared_and_one_of_the_same_net_is_not() {
    let mut b = board(Vec::new());
    b.zones
        .push(zone(2, 2, "solid", "TopCopper", (-1.0, -1.0, 1.0, 1.0)));
    let fill = pieces(&b);
    let hole = RectNm::bounding(fill[0].holes[0].iter().copied()).unwrap();
    assert!(
        hole.min.x <= -3 * MM / 2 && hole.max.x >= 3 * MM / 2,
        "{hole:?}"
    );
    b.zones[1].net = 1;
    assert!(pieces(&b)[0].holes.is_empty());
}

#[test]
fn the_minimum_thickness_drops_a_neck_too_thin_to_make() {
    let mut b = board(Vec::new());
    // Two tracks of net 2 leave a 0.2 mm neck at x = 0 across the pour.
    for (n, y0, y1) in [(7, -20.0, -0.75), (8, 0.75, 20.0)] {
        b.segments.push(Segment {
            uuid: Uuid::from_u128(n),
            start: Point::new(0.0, y0),
            end: Point::new(0.0, y1),
            width: 0.1,
            layer: "TopCopper".into(),
            net: 2,
            unknown_columns: Default::default(),
        });
    }
    // The gap between track ends is 1.5 mm less 0.1 width less 2 x 0.5
    // clearance: about 0.4 mm of copper.
    assert_eq!(pieces(&b).len(), 1, "joined through the neck");
    b.zones[0].min_thickness = 0.6;
    assert_eq!(pieces(&b).len(), 2, "the neck is gone");
}

#[test]
fn only_a_pour_on_a_copper_layer_is_filled() {
    let mut b = board(Vec::new());
    b.zones
        .push(zone(2, 1, "solid", "TopCopper", (0.0, 0.0, 1.0, 1.0)));
    b.zones
        .push(zone(3, 1, POUR_FILL, "TopOverlay", (0.0, 0.0, 1.0, 1.0)));
    let fills = zone_fills(&b);
    assert!(fills.of(0).is_some());
    assert!(fills.of(1).is_none(), "solid copper is not a pour");
    assert!(fills.of(2).is_none(), "no copper layer");
    assert_eq!(fills.iter().count(), 1);
}

#[test]
fn the_pour_clearance_falls_back_to_the_board_then_the_default() {
    let mut b = board(vec![pad(1, 0.0, 2)]);
    b.zones[0].clearance = 0.0;
    let hole_width = |b: &PcbBoard| {
        let fill = pieces(b);
        let r = RectNm::bounding(fill[0].holes[0].iter().copied()).unwrap();
        r.width()
    };
    // Default 0.2 mm: 1 mm pad + 2 x 0.2.
    let w = hole_width(&b);
    assert!((7 * MM / 5..7 * MM / 5 + MM / 50).contains(&w), "{w}");
    b.setup = Some(signex_types::pcb::PcbSetup {
        grid_size: 0.0,
        trace_width: 0.0,
        via_diameter: 0.0,
        via_drill: 0.0,
        clearance: 0.3,
        track_min_width: 0.0,
        via_min_diameter: 0.0,
        via_min_drill: 0.0,
    });
    let w = hole_width(&b);
    assert!((8 * MM / 5..8 * MM / 5 + MM / 50).contains(&w), "{w}");
}
