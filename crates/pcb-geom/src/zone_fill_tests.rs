use super::*;
use crate::rect::RectNm;
use crate::transform::pad_shape;
use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType, Point, Segment};
use signex_types::pcb_arc::TrackArc;
use signex_types::zone_cutout::POUR_CUTOUT_FILL;
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
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
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
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
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
        zones: vec![zone(
            1,
            1,
            POUR_FILL,
            "TopCopper",
            (-10.0, -10.0, 10.0, 10.0),
        )],
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
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
        pad_copper.clone().inflated(clearance - 1),
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
        locked: false,
        attributes: Default::default(),
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

/// #2871: a pour cutout on the pour's layer is left empty as drawn, with no
/// clearance round it, and the pour's outline is not touched; one on another
/// layer is not; moved off the pour or removed, the copper comes back.
#[test]
fn a_pour_cutout_on_the_layer_is_left_empty_and_the_outline_kept() {
    let mut b = board(Vec::new());
    let outline = b.zones[0].outline.clone();
    b.zones.push(zone(
        2,
        0,
        POUR_CUTOUT_FILL,
        "TopCopper",
        (2.0, 2.0, 4.0, 5.0),
    ));
    b.zones.push(zone(
        3,
        0,
        POUR_CUTOUT_FILL,
        "BottomCopper",
        (-8.0, -8.0, -6.0, -6.0),
    ));
    let fill = pieces(&b);
    assert_eq!(fill.len(), 1);
    assert_eq!(fill[0].holes.len(), 1, "the top cutout, not the bottom one");
    assert_eq!(
        twice_area(&fill),
        FULL - 2 * (2 * MM as i128) * (3 * MM as i128)
    );
    assert_eq!(b.zones[0].outline, outline, "the outline is not edited");

    let mut removed = b.clone();
    removed.zones.remove(1);
    assert_eq!(
        twice_area(&pieces(&removed)),
        FULL,
        "removed, the copper is back"
    );
    for p in &mut b.zones[1].outline {
        p.x += 20.0;
    }
    assert_eq!(
        twice_area(&pieces(&b)),
        FULL,
        "moved off, the copper is back"
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
            locked: false,
            attributes: Default::default(),
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

/// A net-2 track along y = `y` across the whole pour.
fn track_across(n: u128, y: f64) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(n),
        start: Point::new(-20.0, y),
        end: Point::new(20.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 2,
        unknown_columns: Default::default(),
    }
}

/// The bounds of every piece of the pour at `index`.
fn fill_bounds(board: &PcbBoard, index: usize) -> RectNm {
    let fills = zone_fills(board);
    let fill = fills.of(index).expect("filled");
    RectNm::bounding(fill.pieces.iter().flat_map(|p| p.outer.iter().copied())).expect("copper")
}

/// #2216: a pour of another net that fills later keeps the clearance from
/// the fill of the pour filled first; a higher priority fills first, and
/// equal priorities fill in board order.
#[test]
fn a_higher_priority_pour_fills_first_and_the_other_keeps_clear_of_it() {
    let mut b = board(Vec::new());
    b.zones
        .push(zone(2, 2, POUR_FILL, "TopCopper", (0.0, -10.0, 20.0, 10.0)));
    // Equal priority: the first pour fills first, the second clears it.
    assert_eq!(twice_area(&pieces(&b)), FULL);
    let second = fill_bounds(&b, 1);
    assert!(
        (21 * MM / 2..21 * MM / 2 + MM / 50).contains(&second.min.x),
        "{second:?}"
    );
    // The second pour on a higher priority fills first.
    b.zones[1].priority = 3;
    let first = fill_bounds(&b, 0);
    assert!(
        (-MM / 2 - MM / 50..=-MM / 2).contains(&first.max.x),
        "{first:?}"
    );
    assert_eq!(fill_bounds(&b, 1).min.x, 0, "the higher one is whole");
}

/// #2216: pours of one net may overlap; neither clears the other.
#[test]
fn pours_of_one_net_overlap() {
    let mut b = board(Vec::new());
    b.zones
        .push(zone(2, 1, POUR_FILL, "TopCopper", (0.0, -10.0, 20.0, 10.0)));
    let fills = zone_fills(&b);
    assert_eq!(twice_area(&fills.of(0).expect("filled").pieces), FULL);
    assert_eq!(twice_area(&fills.of(1).expect("filled").pieces), FULL);
}

/// #2523: a shelved pour has no fill and does not clear other pours; once
/// unshelved it fills and clears again.
#[test]
fn a_shelved_pour_has_no_fill_and_does_not_clear_other_pours() {
    let mut b = board(Vec::new());
    b.zones
        .push(zone(2, 2, POUR_FILL, "TopCopper", (0.0, -10.0, 20.0, 10.0)));
    b.zones[1].priority = 5;
    b.zones[1].shelved = true;
    let fills = zone_fills(&b);
    assert!(fills.of(1).is_none(), "a shelved pour holds no copper");
    assert_eq!(
        twice_area(&fills.of(0).expect("filled").pieces),
        FULL,
        "the other pour fills its whole outline"
    );

    b.zones[1].shelved = false;
    let fills = zone_fills(&b);
    assert_eq!(fills.of(1).expect("filled again").pieces.len(), 1);
    assert!(twice_area(&fills.of(0).expect("filled").pieces) < FULL);

    // Shelving the only pour leaves nothing to fill.
    let mut alone = board(Vec::new());
    alone.zones[0].shelved = true;
    assert!(zone_fills(&alone).is_empty());
}

/// #2216: a pour on another layer does not clear this one.
#[test]
fn a_pour_on_another_layer_is_not_cleared() {
    let mut b = board(Vec::new());
    b.zones.push(zone(
        2,
        2,
        POUR_FILL,
        "BottomCopper",
        (0.0, -10.0, 20.0, 10.0),
    ));
    b.zones[1].priority = 5;
    assert_eq!(twice_area(&pieces(&b)), FULL);
}

/// #2216: a fill stays inside the board outline.
#[test]
fn a_fill_is_clipped_to_the_board_outline() {
    let mut b = board(Vec::new());
    b.outline = vec![
        Point::new(-5.0, -5.0),
        Point::new(5.0, -5.0),
        Point::new(5.0, 5.0),
        Point::new(-5.0, 5.0),
    ];
    let fill = pieces(&b);
    assert_eq!(fill.len(), 1);
    assert_eq!(twice_area(&fill), 2 * (10 * MM as i128) * (10 * MM as i128));
    // No outline: nothing clips it.
    b.outline.clear();
    assert_eq!(twice_area(&pieces(&b)), FULL);
}

/// #2216: a through cutout of the board sketch is left out of the fill; a
/// cutout that does not go through the board is not.
#[test]
fn a_fill_leaves_out_the_boards_through_cutouts() {
    use signex_types::pcb_sketch::{BoardSketch, PcbCutout};

    let mut b = board(Vec::new());
    b.outline = vec![
        Point::new(-5.0, -5.0),
        Point::new(5.0, -5.0),
        Point::new(5.0, 5.0),
        Point::new(-5.0, 5.0),
    ];
    let opening = PcbCutout {
        uuid: Uuid::from_u128(50),
        boundary: vec![
            Point::new(-1.0, -1.0),
            Point::new(1.0, -1.0),
            Point::new(1.0, 1.0),
            Point::new(-1.0, 1.0),
        ],
        edge_radius: 0.0,
        through: true,
    };
    b.sketch = Some(BoardSketch {
        cutouts: vec![opening],
        ..Default::default()
    });
    let (ten, two) = (10 * MM as i128, 2 * MM as i128);
    assert_eq!(twice_area(&pieces(&b)), 2 * (ten * ten - two * two));
    b.sketch.as_mut().expect("sketch").cutouts[0].through = false;
    assert_eq!(twice_area(&pieces(&b)), 2 * ten * ten);
}

/// #2216: an island smaller than the pour's minimum area is dropped and
/// counted; a bigger one stays.
#[test]
fn an_island_below_the_minimum_area_is_dropped() {
    let mut b = board(Vec::new());
    // The track at y = 8 leaves a strip about 20 x 1.375 mm (27.5 mm²) above.
    b.segments.push(track_across(7, 8.0));
    assert_eq!(pieces(&b).len(), 2);
    b.zones[0].min_island_area = 30.0;
    let fills = zone_fills(&b);
    let fill = fills.of(0).expect("filled");
    assert_eq!(fill.pieces.len(), 1);
    assert_eq!(fill.islands_removed, 1);
    b.zones[0].min_island_area = 20.0;
    assert_eq!(pieces(&b).len(), 2, "27.5 mm² is kept");
}

/// #2216: with dead-copper removal, an island touching no copper of the
/// pour's net is dropped; an island on a pad of the net stays. A pour with
/// no net keeps every island.
#[test]
fn an_island_touching_no_copper_of_the_net_is_dropped() {
    let mut b = board(vec![pad(1, 0.0, 1)]);
    b.segments.push(track_across(7, 8.0));
    b.zones[0].remove_dead_islands = true;
    let fills = zone_fills(&b);
    let fill = fills.of(0).expect("filled");
    assert_eq!(fill.pieces.len(), 1, "the strip above the track is dead");
    assert_eq!(fill.islands_removed, 1);
    let kept = RectNm::bounding(fill.pieces[0].outer.iter().copied()).expect("copper");
    assert!(kept.min.y < 0, "the piece holding the pad stays");

    b.zones[0].net = 0;
    assert_eq!(pieces(&b).len(), 2, "no net: nothing is dead");
}

/// #2216: the fill's area in mm².
#[test]
fn the_fill_area_is_in_square_millimetres() {
    let fills = zone_fills(&board(Vec::new()));
    let area = fills.of(0).expect("filled").area_mm2();
    assert!((area - 400.0).abs() < 1e-9, "{area}");
}

/// A free arc (#2373) of `net` on the top copper bowed through `mid`.
fn free_arc(n: u128, net: u32, start: (f64, f64), mid: (f64, f64), end: (f64, f64)) -> TrackArc {
    TrackArc {
        uuid: Uuid::from_u128(n),
        start: Point::new(start.0, start.1),
        mid: Point::new(mid.0, mid.1),
        end: Point::new(end.0, end.1),
        width: 0.2,
        layer: "TopCopper".into(),
        net,
    }
}

/// #2373: a free pad or copper arc of another net inside the pour is
/// cleared like a footprint's copper; one of the pour's own net is not.
#[test]
fn a_free_pad_or_arc_of_another_net_is_cleared() {
    let mut b = board(Vec::new());
    b.free.pads.push(pad(9, -5.0, 2));
    let fill = pieces(&b);
    assert_eq!(fill.len(), 1);
    assert_eq!(fill[0].holes.len(), 1, "the VCC free pad");
    let hole = RectNm::bounding(fill[0].holes[0].iter().copied()).unwrap();
    let w = hole.width();
    assert!(
        (2 * MM..2 * MM + MM / 50).contains(&w),
        "1 mm + 2 x 0.5: {w}"
    );

    b.free
        .arcs
        .push(free_arc(10, 2, (2.0, 0.0), (5.0, 3.0), (8.0, 0.0)));
    assert_eq!(pieces(&b)[0].holes.len(), 2, "and the VCC free arc");

    b.free.pads[0].net.as_mut().unwrap().number = 1;
    b.free.arcs[0].net = 1;
    assert!(pieces(&b)[0].holes.is_empty(), "GND copper is filled over");
}

/// #2401: a pad is cleared at its size on the pour's layer.
#[test]
fn a_pad_is_cleared_at_its_size_on_the_pours_layer() {
    let mut big_on_top = pad(1, 0.0, 2);
    big_on_top.layer_sizes = Some(signex_types::pcb::LayerSizes::TopMiddleBottom {
        top: Point::new(2.0, 2.0),
        middle: Point::new(1.0, 1.0),
        bottom: Point::new(1.0, 1.0),
    });
    let fill = pieces(&board(vec![big_on_top]));
    let hole = RectNm::bounding(fill[0].holes[0].iter().copied()).unwrap();
    let w = hole.width();
    assert!(
        (3 * MM..3 * MM + MM / 50).contains(&w),
        "2 mm + 2 x 0.5: {w}"
    );
}

/// #2373: an island tied to the pour's net only through a free pad or a
/// free copper arc of that net is alive.
#[test]
fn an_island_on_a_free_pad_or_arc_of_the_net_is_kept() {
    let mut b = board(vec![pad(1, 0.0, 1)]);
    b.segments.push(track_across(7, 8.0));
    b.zones[0].remove_dead_islands = true;
    assert_eq!(pieces(&b).len(), 1, "the strip above the track is dead");

    let mut free = pad(9, 0.0, 1);
    free.position = Point::new(0.0, 9.3);
    b.free.pads.push(free);
    assert_eq!(pieces(&b).len(), 2, "the GND free pad keeps the strip");

    b.free.pads.clear();
    b.free
        .arcs
        .push(free_arc(10, 1, (-3.0, 9.3), (0.0, 9.5), (3.0, 9.3)));
    assert_eq!(pieces(&b).len(), 2, "the GND free arc keeps the strip");
}

/// A pour with no fill says why (maintainer 2026-11-18).
#[test]
fn a_pour_with_no_fill_says_why() {
    let mut b = board(Vec::new());
    b.zones = vec![
        zone(1, 1, POUR_FILL, "TopSilkscreen", (0.0, 0.0, 5.0, 5.0)),
        zone(2, 1, POUR_FILL, "TopCopper", (0.0, 0.0, 5.0, 0.0)),
        zone(3, 1, POUR_FILL, "TopCopper", (0.0, 0.0, 5.0, 5.0)),
    ];
    b.zones[1].outline.truncate(2);
    let fills = zone_fills(&b);
    assert_eq!(fills.unfilled(0), Some(Unfilled::NoCopperLayer));
    assert_eq!(fills.unfilled(1), Some(Unfilled::Outline));
    assert_eq!(fills.unfilled(2), None, "the third one fills");
    assert!(fills.of(2).is_some());
    assert_eq!(fills.unfilled_pours().len(), 2);
}
