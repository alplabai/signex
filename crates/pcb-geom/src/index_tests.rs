use super::*;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, ViaType};

const MM: i64 = 1_000_000;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pad(uuid: Uuid, x: f64, y: f64, w: f64, h: f64) -> Pad {
    Pad {
        uuid,
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, y),
        size: Point::new(w, h),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
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

fn footprint(uuid: Uuid, x: f64, y: f64, rotation: f64, layer: &str, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid,
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(x, y),
        rotation,
        layer: layer.into(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn segment(uuid: Uuid, a: (f64, f64), b: (f64, f64), width: f64) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid,
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width,
        layer: "TopCopper".into(),
        net: 1,
        unknown_columns: Default::default(),
    }
}

fn via(uuid: Uuid, x: f64, y: f64, diameter: f64) -> Via {
    Via {
        attributes: Default::default(),
        uuid,
        position: Point::new(x, y),
        diameter,
        drill: diameter / 2.0,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 1,
        locked: false,
        layer_sizes: None,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    }
}

fn board(footprints: Vec<Footprint>, segments: Vec<Segment>, vias: Vec<Via>) -> PcbBoard {
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
        vias,
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

/// U1 at (10, 10) turned 90° with one 2 × 1 mm pad 2 mm out on its x axis,
/// a track from (0, 0) to (20, 0) 0.5 mm wide, a 0.6 mm via at (30, 0).
fn sample() -> PcbBoard {
    let fp = footprint(
        id(1),
        10.0,
        10.0,
        90.0,
        "TopCopper",
        vec![pad(id(2), 2.0, 0.0, 2.0, 1.0)],
    );
    board(
        vec![fp],
        vec![segment(id(3), (0.0, 0.0), (20.0, 0.0), 0.5)],
        vec![via(id(4), 30.0, 0.0, 0.6)],
    )
}

const PAD: ItemRef = ItemRef::Pad {
    footprint: Uuid::from_u128(1),
    pad: Uuid::from_u128(2),
};
const TRACK: ItemRef = ItemRef::Track(Uuid::from_u128(3));
const VIA: ItemRef = ItemRef::Via(Uuid::from_u128(4));

#[test]
fn build_indexes_pads_tracks_and_vias() {
    let index = BoardIndex::build(&sample());
    assert_eq!(index.len(), 3);
    assert!(index.rejected().is_empty());
    assert!(!index.is_empty());
    assert!(BoardIndex::default().is_empty());
}

#[test]
fn point_hits_the_pad_at_its_world_position() {
    let index = BoardIndex::build(&sample());
    // Pad centre: (10, 10) + rot90(2, 0) = (10, 12); its 2 mm side now runs along y.
    assert_eq!(
        index.hit_point(PointNm::new(10 * MM, 12 * MM), 0),
        vec![PAD]
    );
    assert_eq!(
        index.hit_point(PointNm::new(10 * MM, 13 * MM), 0),
        vec![PAD]
    );
    assert!(
        index
            .hit_point(PointNm::new(11 * MM, 12 * MM), 0)
            .is_empty()
    );
}

#[test]
fn point_hits_tracks_and_vias_with_tolerance() {
    let index = BoardIndex::build(&sample());
    assert_eq!(
        index.hit_point(PointNm::new(5 * MM, MM / 4), 0),
        vec![TRACK]
    );
    assert!(
        index
            .hit_point(PointNm::new(5 * MM, MM / 4 + 10), 0)
            .is_empty()
    );
    assert_eq!(
        index.hit_point(PointNm::new(5 * MM, MM / 4 + 10), 10),
        vec![TRACK]
    );
    assert_eq!(
        index.hit_point(PointNm::new(30 * MM, 300_000), 0),
        vec![VIA]
    );
    assert_eq!(
        index.hit_point_mm(Point::new(30.0, 0.35), 0.05),
        Ok(vec![VIA])
    );
    assert!(index.hit_point_mm(Point::new(f64::NAN, 0.0), 0.0).is_err());
}

#[test]
fn overlapping_items_come_back_sorted() {
    let b = board(
        Vec::new(),
        vec![segment(id(9), (-1.0, 0.0), (1.0, 0.0), 0.2)],
        vec![via(id(8), 0.0, 0.0, 0.6)],
    );
    let index = BoardIndex::build(&b);
    assert_eq!(
        index.hit_point(PointNm::ORIGIN, 0),
        vec![ItemRef::Track(id(9)), ItemRef::Via(id(8))]
    );
}

#[test]
fn rect_inside_needs_all_copper_touching_needs_any() {
    let index = BoardIndex::build(&sample());
    let around_via = RectNm::new(PointNm::new(29 * MM, -MM), PointNm::new(31 * MM, MM));
    assert_eq!(index.hit_rect(&around_via, RectSelect::Inside), vec![VIA]);
    // The track's last 1 mm and the whole via.
    let tail = RectNm::new(PointNm::new(19 * MM, -MM), PointNm::new(31 * MM, MM));
    assert_eq!(index.hit_rect(&tail, RectSelect::Inside), vec![VIA]);
    assert_eq!(
        index.hit_rect(&tail, RectSelect::Touching),
        vec![TRACK, VIA]
    );
    let empty = RectNm::new(
        PointNm::new(40 * MM, 40 * MM),
        PointNm::new(41 * MM, 41 * MM),
    );
    assert!(index.hit_rect(&empty, RectSelect::Touching).is_empty());
}

#[test]
fn bottom_side_pad_is_mirrored_before_hit_test() {
    let fp = footprint(
        id(1),
        0.0,
        0.0,
        0.0,
        "BottomCopper",
        vec![pad(id(2), 2.0, 0.0, 1.0, 1.0)],
    );
    let index = BoardIndex::build(&board(vec![fp], Vec::new(), Vec::new()));
    assert_eq!(index.hit_point(PointNm::new(-2 * MM, 0), 0), vec![PAD]);
    assert!(index.hit_point(PointNm::new(2 * MM, 0), 0).is_empty());
}

#[test]
fn unconvertible_items_are_reported_not_dropped_silently() {
    let b = board(
        vec![footprint(
            id(1),
            0.0,
            0.0,
            f64::NAN,
            "TopCopper",
            vec![pad(id(2), 0.0, 0.0, 1.0, 1.0)],
        )],
        vec![segment(id(3), (0.0, 0.0), (f64::INFINITY, 0.0), 0.2)],
        vec![via(id(4), 0.0, 0.0, 0.6)],
    );
    let index = BoardIndex::build(&b);
    assert_eq!(index.len(), 1);
    assert_eq!(
        index.rejected(),
        &[
            Rejected {
                item: PAD,
                error: GeomError::Unrepresentable("footprint.rotation"),
            },
            Rejected {
                item: TRACK,
                error: GeomError::Unrepresentable("track.end"),
            },
        ]
    );
}

#[test]
fn many_items_query_the_same_as_a_linear_scan() {
    let vias: Vec<Via> = (0..400)
        .map(|i| {
            via(
                id(1000 + i),
                (i % 20) as f64 * 1.5,
                (i / 20) as f64 * 1.5,
                1.0,
            )
        })
        .collect();
    let index = BoardIndex::build(&board(Vec::new(), Vec::new(), vias));
    let probe = RectNm::new(PointNm::new(3 * MM, 3 * MM), PointNm::new(9 * MM, 6 * MM));
    let scan: Vec<ItemRef> = {
        let mut v: Vec<ItemRef> = index
            .items()
            .filter(|e| e.shape.touches_rect(&probe))
            .map(|e| e.item)
            .collect();
        v.sort_unstable();
        v
    };
    assert!(!scan.is_empty());
    assert_eq!(index.hit_rect(&probe, RectSelect::Touching), scan);
}

#[test]
fn from_items_indexes_shapes_given_in_nm() {
    let item = IndexedItem::new(
        VIA,
        Shape::Disc {
            center: PointNm::new(MM, MM),
            radius: 100,
        },
    )
    .unwrap();
    let index = BoardIndex::from_items(vec![item]);
    assert_eq!(index.hit_point(PointNm::new(MM + 100, MM), 0), vec![VIA]);
}

#[test]
fn shapes_beyond_the_limit_are_refused() {
    let far = Shape::Disc {
        center: PointNm::new(i64::MAX / 2, 0),
        radius: 1,
    };
    assert_eq!(
        IndexedItem::new(VIA, far),
        Err(GeomError::Unrepresentable("shape"))
    );
}

#[test]
fn extreme_query_inputs_do_not_overflow() {
    let index = BoardIndex::build(&sample());
    assert!(
        index
            .hit_point(PointNm::new(i64::MIN, i64::MAX), 0)
            .is_empty()
    );
    assert!(
        index
            .hit_point(PointNm::new(0, 0), i64::MAX)
            .contains(&TRACK)
    );
    assert!(
        index
            .hit_point(PointNm::new(i64::MAX, i64::MAX), i64::MAX)
            .is_empty()
    );
    let everything = RectNm::new(
        PointNm::new(i64::MIN, i64::MIN),
        PointNm::new(i64::MAX, i64::MAX),
    );
    assert!(
        index
            .hit_rect(&everything, RectSelect::Touching)
            .contains(&TRACK)
    );
    assert!(
        index
            .hit_rect(&everything, RectSelect::Inside)
            .contains(&TRACK)
    );
}

#[test]
fn a_repeated_item_identity_is_rejected() {
    let v = via(id(4), 30.0, 0.0, 0.6);
    let other = via(id(4), 31.0, 0.0, 0.6);
    let index = BoardIndex::build(&board(Vec::new(), Vec::new(), vec![v, other]));
    assert_eq!(index.len(), 1);
    assert_eq!(
        index.rejected(),
        &[Rejected {
            item: VIA,
            error: GeomError::DuplicateItem
        }]
    );
}

fn area(uuid: Uuid, fill: &str, outline: &[(f64, f64)]) -> Zone {
    Zone {
        uuid,
        net: 0,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: outline.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        holes: Vec::new(),
        priority: 0,
        fill_type: fill.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        unknown: Default::default(),
    }
}

/// #2383: a keepout is picked on its edge and inside it, a box selects it
/// whole only when it holds every edge; #2216: a pour is picked inside its
/// outline as a polygon.
#[test]
fn drawn_areas_are_picked_on_their_edges_and_inside() {
    let mut b = board(Vec::new(), Vec::new(), Vec::new());
    let triangle = [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)];
    b.zones = vec![
        area(id(7), "keepout", &triangle),
        area(id(8), "pour", &[(20.0, 0.0), (30.0, 0.0), (30.0, 10.0)]),
    ];
    let index = BoardIndex::build_selectable(&b);
    let keepout = ItemRef::Zone {
        zone: id(7),
        kind: PickKind::Keepouts,
    };
    assert_eq!(
        index.hit_point(PointNm::new(2 * MM, 2 * MM), 0),
        vec![keepout]
    );
    assert_eq!(index.hit_point(PointNm::new(5 * MM, 0), 0), vec![keepout]);
    assert!(index.hit_point(PointNm::new(8 * MM, 8 * MM), 0).is_empty());
    let pour = ItemRef::Zone {
        zone: id(8),
        kind: PickKind::Polygons,
    };
    assert_eq!(
        index.hit_point(PointNm::new(25 * MM, 2 * MM), 0),
        vec![pour]
    );
    assert!(index.hit_point(PointNm::new(21 * MM, 8 * MM), 0).is_empty());
    let whole = RectNm::new(PointNm::new(-MM, -MM), PointNm::new(11 * MM, 11 * MM));
    let half = RectNm::new(PointNm::new(-MM, -MM), PointNm::new(5 * MM, 11 * MM));
    let within = RectNm::new(PointNm::new(MM, MM), PointNm::new(2 * MM, 2 * MM));
    assert_eq!(index.hit_rect(&whole, RectSelect::Inside), vec![keepout]);
    assert!(index.hit_rect(&half, RectSelect::Inside).is_empty());
    assert_eq!(index.hit_rect(&half, RectSelect::Touching), vec![keepout]);
    assert_eq!(index.hit_rect(&within, RectSelect::Touching), vec![keepout]);
    assert!(
        BoardIndex::build(&b).is_empty(),
        "connectivity holds no areas"
    );
}

/// #2403: every kind of item is found by its uuid; a pour and an unknown
/// uuid are not.
#[test]
fn an_item_is_found_by_its_uuid() {
    let fp = footprint(
        id(1),
        0.0,
        0.0,
        0.0,
        "TopCopper",
        vec![pad(id(2), 0.0, 0.0, 1.0, 1.0)],
    );
    let mut b = board(
        vec![fp],
        vec![segment(id(3), (0.0, 0.0), (1.0, 0.0), 0.2)],
        vec![via(id(4), 0.0, 0.0, 0.6)],
    );
    b.zones = vec![
        area(id(5), "keepout", &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
        area(id(6), "pour", &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
    ];
    assert_eq!(item_by_uuid(&b, id(1)), Some(ItemRef::Footprint(id(1))));
    assert_eq!(
        item_by_uuid(&b, id(2)),
        Some(ItemRef::Pad {
            footprint: id(1),
            pad: id(2)
        })
    );
    assert_eq!(item_by_uuid(&b, id(3)), Some(ItemRef::Track(id(3))));
    assert_eq!(item_by_uuid(&b, id(4)), Some(ItemRef::Via(id(4))));
    assert_eq!(
        item_by_uuid(&b, id(5)),
        Some(ItemRef::Zone {
            zone: id(5),
            kind: PickKind::Keepouts
        })
    );
    // A pour is pickable since #2216.
    assert_eq!(
        item_by_uuid(&b, id(6)),
        Some(ItemRef::Zone {
            zone: id(6),
            kind: PickKind::Polygons
        })
    );
    assert_eq!(item_by_uuid(&b, id(9)), None);
}

/// #899: a click or box inside a zone's hole does not pick the zone, while
/// the copper around the hole and the hole's edge do. A hole the fill cannot
/// use (one crossing the outline) is copper and does not cut the pick.
#[test]
fn a_zone_is_not_picked_inside_its_holes() {
    let mut b = board(Vec::new(), Vec::new(), Vec::new());
    let square = |x: f64, y: f64, side: f64| {
        vec![
            Point::new(x, y),
            Point::new(x + side, y),
            Point::new(x + side, y + side),
            Point::new(x, y + side),
        ]
    };
    let mut pour = area(
        id(7),
        "pour",
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
    );
    pour.holes = vec![square(4.0, 4.0, 2.0), square(-1.0, 7.0, 2.0)];
    assert_eq!(
        pour.usable_holes(),
        vec![0],
        "the second hole crosses the outline"
    );
    b.zones = vec![pour];
    let index = BoardIndex::build_selectable(&b);
    let zone = ItemRef::Zone {
        zone: id(7),
        kind: PickKind::Polygons,
    };
    let at = |x: f64, y: f64| PointNm::new((x * MM as f64) as i64, (y * MM as f64) as i64);
    assert_eq!(index.hit_point(at(2.0, 2.0), 0), vec![zone], "the copper");
    assert!(
        index.hit_point(at(5.0, 5.0), 0).is_empty(),
        "inside the hole"
    );
    assert!(
        index.hit_point(at(5.0, 5.0), MM / 10).is_empty(),
        "inside the hole, the edge out of reach"
    );
    assert_eq!(
        index.hit_point(at(4.0, 5.0), 0),
        vec![zone],
        "the hole's edge"
    );
    assert_eq!(
        index.hit_point(at(4.05, 5.0), MM / 10),
        vec![zone],
        "within reach of the hole's edge"
    );
    assert_eq!(
        index.hit_point(at(0.5, 8.0), 0),
        vec![zone],
        "an unusable hole is copper"
    );

    let in_hole = RectNm::new(at(4.5, 4.5), at(5.5, 5.5));
    assert!(index.hit_rect(&in_hole, RectSelect::Touching).is_empty());
    let out_of_hole = RectNm::new(at(4.5, 4.5), at(7.0, 5.5));
    assert_eq!(
        index.hit_rect(&out_of_hole, RectSelect::Touching),
        vec![zone],
        "a box from the hole onto the copper"
    );
    let on_copper = RectNm::new(at(1.0, 1.0), at(2.0, 2.0));
    assert_eq!(index.hit_rect(&on_copper, RectSelect::Touching), vec![zone]);
    let whole = RectNm::new(at(-2.0, -1.0), at(11.0, 11.0));
    assert_eq!(index.hit_rect(&whole, RectSelect::Inside), vec![zone]);
    let outline_only = RectNm::new(at(-1.0, -1.0), at(11.0, 3.0));
    assert!(index.hit_rect(&outline_only, RectSelect::Inside).is_empty());
}

#[test]
fn a_box_never_takes_a_footprint_text_and_a_point_does() {
    let mut part = footprint(id(1), 10.0, 10.0, 0.0, "TopCopper", Vec::new());
    part.properties.push(signex_types::property::PcbProperty {
        key: "Reference".into(),
        value: "R1".into(),
        position: Some(Point::new(0.0, -2.0)),
        ..Default::default()
    });
    let index = BoardIndex::build_selectable(&board(vec![part], Vec::new(), Vec::new()));
    let text = ItemRef::FootprintText {
        footprint: id(1),
        field: BoardTextField::Designator,
    };
    assert!(
        index
            .hit_point(PointNm::new(10 * MM, 8 * MM), 0)
            .contains(&text)
    );
    let rect = RectNm::new(PointNm::new(0, 0), PointNm::new(20 * MM, 20 * MM));
    let square = [
        PointNm::new(0, 0),
        PointNm::new(20 * MM, 0),
        PointNm::new(20 * MM, 20 * MM),
        PointNm::new(0, 20 * MM),
    ];
    for mode in [RectSelect::Inside, RectSelect::Touching] {
        let boxed = index.hit_rect(&rect, mode);
        assert!(boxed.contains(&ItemRef::Footprint(id(1))), "{boxed:?}");
        assert!(!boxed.contains(&text), "{boxed:?}");
        let lassoed = index.hit_polygon(&square, mode);
        assert!(lassoed.contains(&ItemRef::Footprint(id(1))), "{lassoed:?}");
        assert!(!lassoed.contains(&text), "{lassoed:?}");
    }
}
