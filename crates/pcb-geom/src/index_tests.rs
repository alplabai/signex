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
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn segment(uuid: Uuid, a: (f64, f64), b: (f64, f64), width: f64) -> Segment {
    Segment {
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
        uuid,
        position: Point::new(x, y),
        diameter,
        drill: diameter / 2.0,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 1,
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
    assert!(index.hit_point(PointNm::new(i64::MIN, i64::MAX), 0).is_empty());
    assert!(index.hit_point(PointNm::new(0, 0), i64::MAX).contains(&TRACK));
    assert!(
        index
            .hit_point(PointNm::new(i64::MAX, i64::MAX), i64::MAX)
            .is_empty()
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
