use super::*;
use signex_types::net_class::NetClass;
use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, PcbSetup, Point, Segment, Via,
    ViaType,
};
use uuid::Uuid;

const MM: i64 = 1_000_000;
/// A 0.25 mm track is 125_000 nm either side of its centre line.
const HALF: i64 = 125_000;
const RULE: i64 = 200_000;

pub(super) fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

pub(super) fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32, layer: &str) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: id(2000 + n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: layer.into(),
        net,
        unknown_columns: Default::default(),
    }
}

pub(super) fn via(n: u128, at: (f64, f64), net: u32) -> Via {
    Via {
        attributes: Default::default(),
        uuid: id(3000 + n),
        position: Point::new(at.0, at.1),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net,
        locked: false,
        layer_sizes: None,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    }
}

pub(super) fn pad(n: u128, at: (f64, f64), net: u32, pad_type: PadType, layer: &str) -> Pad {
    Pad {
        uuid: id(n),
        number: n.to_string(),
        pad_type,
        shape: PadShape::Rect,
        position: Point::new(at.0, at.1),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec![layer.into()],
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

pub(super) fn footprint(n: u128, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid: id(1000 + n),
        reference: format!("U{n}"),
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

pub(super) fn board(
    footprints: Vec<Footprint>,
    segments: Vec<Segment>,
    vias: Vec<Via>,
) -> PcbBoard {
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

/// Two parallel tracks on nets 1 and 2, `pitch_mm` apart centre to centre.
fn parallel(pitch_mm: f64, layer_b: &str) -> PcbBoard {
    board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (0.0, pitch_mm), (10.0, pitch_mm), 2, layer_b),
        ],
        vec![],
    )
}

fn kinds(report: &DrcReport) -> Vec<ViolationKind> {
    report.violations.iter().map(|v| v.kind).collect()
}

#[test]
fn a_gap_of_exactly_the_rule_passes_and_one_nm_under_fails() {
    // Pitch 0.45 mm less two half widths is 200_000 nm.
    let rules = DrcRules::new(RULE);
    assert_eq!(2 * HALF + RULE, 450_000);
    assert!(run(&parallel(0.45, "TopCopper"), &rules).is_clean());

    let report = run(&parallel(0.449_999, "TopCopper"), &rules);
    assert_eq!(kinds(&report), [ViolationKind::Clearance]);
    let v = &report.violations[0];
    assert_eq!((v.measured_nm, v.required_nm), (RULE - 1, RULE));
    assert_eq!(v.nets, (1, 2));
    assert_eq!(
        v.items,
        [ItemRef::Track(id(2001)), ItemRef::Track(id(2002))]
    );
    // Midway between the two centre lines' nearest points.
    assert_eq!(v.at.y, 224_999);
}

#[test]
fn tracks_of_one_net_never_conflict() {
    let mut b = parallel(0.3, "TopCopper");
    b.segments[1].net = 1;
    assert!(run(&b, &DrcRules::new(RULE)).violations.is_empty());
}

#[test]
fn copper_on_other_layers_does_not_conflict_but_a_through_item_reaches_all() {
    let rules = DrcRules::new(RULE);
    // Top track over a bottom track: no shared layer.
    assert!(
        run(&parallel(0.0, "BottomCopper"), &rules)
            .violations
            .is_empty()
    );

    // A through-hole pad that lists only the top layer still reaches the
    // bottom one.
    let thru = footprint(1, vec![pad(1, (5.0, 0.0), 1, PadType::Thru, "TopCopper")]);
    let near_bottom = track(1, (0.0, 0.8), (10.0, 0.8), 2, "BottomCopper");
    let b = board(vec![thru], vec![near_bottom.clone()], vec![]);
    let report = run(&b, &rules);
    // The pad's edge is at y = 0.5 mm, the track copper from 0.675 mm.
    assert_eq!(kinds(&report), [ViolationKind::Clearance]);
    assert_eq!(report.violations[0].measured_nm, 175_000);
    assert_eq!(report.violations[0].layers, 1 << 31);

    // The same pad as a surface pad on top does not meet a bottom track.
    let smd = footprint(1, vec![pad(1, (5.0, 0.0), 1, PadType::Smd, "TopCopper")]);
    assert!(run(&board(vec![smd], vec![near_bottom], vec![]), &rules).is_clean());

    // A through via (edge at 0.3 mm) reaches the bottom layer too.
    let b = board(
        vec![],
        vec![track(1, (0.0, 0.6), (10.0, 0.6), 2, "BottomCopper")],
        vec![via(1, (5.0, 0.0), 1)],
    );
    assert_eq!(kinds(&run(&b, &rules)), [ViolationKind::Clearance]);
}

#[test]
fn copper_of_two_nets_that_overlaps_or_touches_is_a_short_not_a_clearance() {
    let rules = DrcRules::new(RULE);
    let cross = board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (5.0, -5.0), (5.0, 5.0), 2, "TopCopper"),
        ],
        vec![],
    );
    let report = run(&cross, &rules);
    assert_eq!(kinds(&report), [ViolationKind::Short]);
    assert_eq!(report.violations[0].measured_nm, 0);
    assert_eq!(report.violations[0].at, PointNm::new(5 * MM, 0));

    // Edges that just meet: pitch 0.25 mm is two half widths.
    let touching = run(&parallel(0.25, "TopCopper"), &rules);
    assert_eq!(kinds(&touching), [ViolationKind::Short]);
    // One nm of air is a clearance, not a short.
    let apart = run(&parallel(0.250_001, "TopCopper"), &rules);
    assert_eq!(kinds(&apart), [ViolationKind::Clearance]);
}

#[test]
fn shorts_sort_before_clearances_so_a_row_cap_keeps_them() {
    let rules = DrcRules::new(RULE);
    // Many clearance pairs, then one crossing short added last.
    let mut segments: Vec<Segment> = (0..20)
        .map(|i| {
            track(
                10 + i,
                (0.0, i as f64 * 0.3),
                (10.0, i as f64 * 0.3),
                1 + (i as u32 % 2),
                "TopCopper",
            )
        })
        .collect();
    segments.push(track(100, (20.0, 0.0), (30.0, 0.0), 7, "TopCopper"));
    segments.push(track(101, (25.0, -5.0), (25.0, 5.0), 8, "TopCopper"));
    let report = run(&board(vec![], segments, vec![]), &rules);
    assert!(report.count(ViolationKind::Clearance) > 1, "{report:?}");
    assert_eq!(report.violations[0].kind, ViolationKind::Short);
}

#[test]
fn a_net_class_raises_the_rule_for_its_pairs_only() {
    let mut rules = DrcRules::new(RULE);
    // Pitch 0.55 mm: gap 300_000 nm, over the board rule.
    let b = parallel(0.55, "TopCopper");
    assert!(run(&b, &rules).is_clean());
    rules.net_clearance.insert(2, 350_000);
    let report = run(&b, &rules);
    assert_eq!(kinds(&report), [ViolationKind::Clearance]);
    assert_eq!(report.violations[0].required_nm, 350_000);
    assert_eq!(rules.required(1, 3), RULE);
    assert_eq!(rules.required(3, 2), 350_000);
}

#[test]
fn an_open_net_is_one_violation_per_missing_link_naming_its_pads() {
    let rules = DrcRules::new(RULE);
    let fp = footprint(
        1,
        vec![
            pad(1, (0.0, 0.0), 1, PadType::Smd, "TopCopper"),
            pad(2, (10.0, 0.0), 1, PadType::Smd, "TopCopper"),
        ],
    );
    let open = run(&board(vec![fp.clone()], vec![], vec![]), &rules);
    assert_eq!(kinds(&open), [ViolationKind::Unconnected]);
    let v = &open.violations[0];
    assert_eq!(v.nets, (1, 1));
    assert_eq!(v.measured_nm, 10 * MM);
    assert_eq!(v.at, PointNm::new(5 * MM, 0));
    assert_eq!(v.items.len(), 2);

    let routed = board(
        vec![fp],
        vec![track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper")],
        vec![],
    );
    assert!(run(&routed, &rules).is_clean());
}

#[test]
fn each_pair_is_reported_once_and_the_order_is_fixed() {
    let rules = DrcRules::new(RULE);
    let mut b = parallel(0.4, "TopCopper");
    b.segments
        .push(track(3, (0.0, 0.8), (10.0, 0.8), 3, "TopCopper"));
    let first = run(&b, &rules);
    assert_eq!(first.count(ViolationKind::Clearance), 2);
    b.segments.reverse();
    assert_eq!(run(&b, &rules).violations, first.violations);
}

#[test]
fn a_board_that_passes_starts_failing_when_one_track_moves_one_nm_closer() {
    let rules = DrcRules::new(RULE);
    let mut b = parallel(0.45, "TopCopper");
    assert!(run(&b, &rules).is_clean());
    b.segments[1].start.y -= 0.000_001;
    b.segments[1].end.y -= 0.000_001;
    assert_eq!(kinds(&run(&b, &rules)), [ViolationKind::Clearance]);
}

#[test]
fn an_item_that_does_not_convert_is_listed_as_skipped() {
    let mut b = parallel(0.45, "TopCopper");
    b.segments[0].start = Point::new(f64::NAN, 0.0);
    let report = run(&b, &DrcRules::new(RULE));
    assert_eq!(report.skipped.len(), 1);
    assert!(!report.is_clean());
}

#[test]
fn the_rules_come_from_the_setup_the_default_and_the_net_classes() {
    let mut b = parallel(0.45, "TopCopper");
    let default = DrcRules::for_board(&b, &[]).unwrap();
    assert_eq!(default.clearance, 200_000);
    assert!(default.from_default);
    assert_eq!(DEFAULT_CLEARANCE_MM, 0.2);

    b.setup = Some(PcbSetup {
        grid_size: 0.0,
        trace_width: 0.0,
        via_diameter: 0.0,
        via_drill: 0.0,
        clearance: 0.3,
        track_min_width: 0.0,
        via_min_diameter: 0.0,
        via_min_drill: 0.0,
    });
    b.nets = vec![
        NetDef {
            number: 1,
            name: "GND".into(),
        },
        NetDef {
            number: 2,
            name: "HV_A".into(),
        },
    ];
    let mut hv = NetClass::named("HV");
    hv.patterns = vec!["HV_*".into()];
    hv.clearance = 0.5;
    let rules = DrcRules::for_board(&b, &[hv]).unwrap();
    assert_eq!(rules.clearance, 300_000);
    assert!(!rules.from_default);
    assert_eq!(rules.net_clearance.get(&2), Some(&500_000));
    assert_eq!(rules.net_clearance.get(&1), None);

    b.setup.as_mut().unwrap().clearance = f64::NAN;
    assert!(DrcRules::for_board(&b, &[]).is_err());
}

fn blind_via(n: u128, at: (f64, f64), net: u32, bottom: &str, via_type: ViaType) -> Via {
    Via {
        attributes: Default::default(),
        layers: vec!["TopCopper".into(), bottom.into()],
        locked: false,
        layer_sizes: None,
        via_type,
        ..via(n, at, net)
    }
}

#[test]
fn a_blind_or_micro_via_reaches_only_the_layers_it_spans() {
    let rules = DrcRules::new(RULE);
    for via_type in [ViaType::Blind, ViaType::Micro] {
        // Spans Top to Inner2; its edge is at 0.3 mm, a track at 0.5 mm
        // has copper from 0.375 mm: 0.075 mm apart, under the rule.
        let via = blind_via(1, (5.0, 0.0), 1, "Inner2Copper", via_type);
        let track_on = |layer: &str| track(1, (0.0, 0.5), (10.0, 0.5), 2, layer);
        let beyond = board(vec![], vec![track_on("Inner3Copper")], vec![via.clone()]);
        assert!(
            run(&beyond, &rules).is_clean(),
            "{via_type:?} below its span"
        );
        let inside = board(vec![], vec![track_on("Inner1Copper")], vec![via.clone()]);
        let report = run(&inside, &rules);
        assert_eq!(kinds(&report), [ViolationKind::Clearance], "{via_type:?}");
        // Inner1 is bit 1.
        assert_eq!(report.violations[0].layers, 1 << 1);
        let at_end = board(vec![], vec![track_on("Inner2Copper")], vec![via]);
        assert_eq!(kinds(&run(&at_end, &rules)), [ViolationKind::Clearance]);
    }
}

#[test]
fn a_via_is_checked_with_its_size_on_each_layer() {
    // 0.6 mm outside, 0.2 mm on the inner layers (#2401). A track at 0.5 mm
    // has copper from 0.375 mm: 0.075 mm from the outer ring, 0.275 mm
    // from the inner one.
    let rules = DrcRules::new(RULE);
    let mut small_inside = via(1, (5.0, 0.0), 1);
    small_inside.layer_sizes = Some(signex_types::pcb::LayerSizes::TopMiddleBottom {
        top: 0.6,
        middle: 0.2,
        bottom: 0.6,
    });
    let track_on = |layer: &str| track(1, (0.0, 0.5), (10.0, 0.5), 2, layer);
    let inner = board(
        vec![],
        vec![track_on("Inner1Copper")],
        vec![small_inside.clone()],
    );
    assert!(run(&inner, &rules).is_clean(), "the inner ring is clear");
    let top = board(vec![], vec![track_on("TopCopper")], vec![small_inside]);
    let report = run(&top, &rules);
    assert_eq!(kinds(&report), [ViolationKind::Clearance]);
    assert_eq!(report.violations[0].layers, 1, "on the top layer only");
}

#[test]
fn a_pad_is_checked_with_its_size_on_each_layer() {
    // A through pad at (5, 0): 1 mm outside, 1.6 mm on the inner layers
    // (#2401). A track at y = 1 mm has copper from 0.875 mm: 0.375 mm from
    // the outer copper's edge, 0.075 mm from the inner one's.
    let rules = DrcRules::new(RULE);
    let mut big_inside = pad(1, (5.0, 0.0), 1, PadType::Thru, "TopCopper");
    big_inside.layer_sizes = Some(signex_types::pcb::LayerSizes::TopMiddleBottom {
        top: Point::new(1.0, 1.0),
        middle: Point::new(1.6, 1.6),
        bottom: Point::new(1.0, 1.0),
    });
    let part = footprint(1, vec![big_inside]);
    let track_on = |layer: &str| track(1, (0.0, 1.0), (10.0, 1.0), 2, layer);
    let top = board(vec![part.clone()], vec![track_on("TopCopper")], vec![]);
    assert!(run(&top, &rules).is_clean(), "the base size is clear");
    let inner = board(vec![part], vec![track_on("Inner1Copper")], vec![]);
    let report = run(&inner, &rules);
    assert_eq!(kinds(&report), [ViolationKind::Clearance]);
    let v = &report.violations[0];
    assert_eq!(v.layers, 1 << 1, "on the inner layer only");
    assert!((74_000..=76_000).contains(&v.measured_nm), "{v:?}");
}

fn pad_sized(size: (f64, f64), shape: PadShape) -> Footprint {
    let mut p = pad(1, (5.0, 0.0), 1, PadType::Smd, "TopCopper");
    p.size = Point::new(size.0, size.1);
    p.shape = shape;
    footprint(1, vec![p])
}

#[test]
fn a_zero_size_pad_is_a_point_and_a_zero_width_pad_a_line() {
    let rules = DrcRules::new(RULE);
    // A point at (5, 0); a track 0.2 mm away has copper from 0.075 mm.
    let point = board(
        vec![pad_sized((0.0, 0.0), PadShape::Rect)],
        vec![track(1, (0.0, 0.2), (10.0, 0.2), 2, "TopCopper")],
        vec![],
    );
    let report = run(&point, &rules);
    assert_eq!(kinds(&report), [ViolationKind::Clearance]);
    assert_eq!(report.violations[0].measured_nm, 75_000);
    // The same point 0.4 mm away is clear.
    let clear = board(
        vec![pad_sized((0.0, 0.0), PadShape::Rect)],
        vec![track(1, (0.0, 0.4), (10.0, 0.4), 2, "TopCopper")],
        vec![],
    );
    assert!(run(&clear, &rules).is_clean());

    // A line from (5, -0.5) to (5, 0.5) crossed by a track is a short; its
    // line does not stretch past its ends.
    let line = pad_sized((0.0, 1.0), PadShape::Rect);
    let crossed = board(
        vec![line.clone()],
        vec![track(1, (0.0, 0.0), (10.0, 0.0), 2, "TopCopper")],
        vec![],
    );
    assert_eq!(kinds(&run(&crossed, &rules)), [ViolationKind::Short]);
    let past = board(
        vec![line],
        vec![track(1, (0.0, 2.0), (10.0, 2.0), 2, "TopCopper")],
        vec![],
    );
    assert!(run(&past, &rules).is_clean());
}

#[test]
fn a_round_rect_pad_is_checked_with_its_rounded_corner() {
    // Pads are measured on their real outline (#680): a round-rect's corner
    // is farther from a diagonal track than a sharp rectangle's.
    let rules = DrcRules::new(RULE);
    // A track along x + y = 6.4, about 0.16 mm of air from the sharp corner
    // (5.5, 0.5) but about 0.37 mm from a fully rounded one.
    let diagonal = vec![track(1, (5.0, 1.4), (7.0, -0.6), 2, "TopCopper")];
    let with = |shape, ratio| {
        let mut fp = pad_sized((1.0, 1.0), shape);
        fp.pads[0].roundrect_ratio = ratio;
        board(vec![fp], diagonal.clone(), vec![])
    };
    let sharp = run(&with(PadShape::Rect, 0.0), &rules);
    assert_eq!(kinds(&sharp), [ViolationKind::Clearance], "{sharp:?}");
    // A ratio of 0 is a plain rectangle.
    assert_eq!(
        run(&with(PadShape::RoundRect, 0.0), &rules).violations,
        sharp.violations
    );
    let round = run(&with(PadShape::RoundRect, 0.5), &rules);
    let gap = |r: &DrcReport| r.violations.first().map_or(i64::MAX, |v| v.measured_nm);
    assert!(gap(&round) > gap(&sharp), "{round:?} vs {sharp:?}");
}

#[test]
fn an_oval_pad_is_measured_to_its_tip_exactly_and_off_axis_by_a_32_gon() {
    // The tip of the polygon is a vertex, so the gap along the axis is exact.
    let rules = DrcRules::new(RULE);
    let oval = |x: f64| {
        board(
            vec![pad_sized((2.0, 1.0), PadShape::Oval)],
            // Vertical track; tip of the pad at x = 6.0 mm; half width 0.125 mm.
            vec![track(1, (x, -3.0), (x, 3.0), 2, "TopCopper")],
            vec![],
        )
    };
    assert!(run(&oval(6.325), &rules).is_clean());
    assert_eq!(
        kinds(&run(&oval(6.324_999), &rules)),
        [ViolationKind::Clearance]
    );
}

/// #2389: the router asks a pair's clearance as the check measures it: the
/// stricter net's class, and nothing between items on no shared layer.
#[test]
fn the_clearance_between_two_items_is_what_the_check_asks_of_them() {
    let mut rules = DrcRules::new(RULE);
    rules.net_clearance.insert(2, 350_000);
    let item = |net: u32, layers: u32| pcb_geom::CopperItem {
        item: pcb_geom::ItemRef::Track(id(net.into())),
        net,
        layers,
        shape: pcb_geom::Shape::Disc {
            center: pcb_geom::PointNm::ORIGIN,
            radius: 0,
        },
        pad_center: None,
    };
    assert_eq!(rules.clearance_between(&item(1, 1), &item(3, 1)), RULE);
    assert_eq!(rules.clearance_between(&item(1, 1), &item(2, 1)), 350_000);
    assert_eq!(rules.clearance_between(&item(1, 1), &item(2, 2)), 0);
    assert_eq!(rules.track_width_limits(1, 1), (None, None));
}

/// #2373: a copper arc is checked along every chord, and a track beside it
/// is one finding, not one per chord it comes near.
#[test]
fn a_track_beside_the_middle_of_an_arc_is_one_clearance_finding() {
    let mut b = board(
        vec![],
        vec![track(1, (4.0, 5.5), (6.0, 5.5), 2, "TopCopper")],
        vec![],
    );
    b.free.arcs.push(signex_types::pcb_arc::TrackArc {
        uuid: id(4000),
        start: Point::new(0.0, 0.0),
        mid: Point::new(5.0, 5.0),
        end: Point::new(10.0, 0.0),
        width: 0.5,
        layer: "TopCopper".into(),
        net: 1,
    });
    let report = run(&b, &DrcRules::new(RULE));
    assert_eq!(kinds(&report), [ViolationKind::Clearance]);
    assert!(
        report.violations[0]
            .items
            .contains(&pcb_geom::ItemRef::Arc(id(4000)))
    );
}

#[test]
fn a_trapezoid_pad_is_measured_to_its_long_edge_corner() {
    // #2242. A 1 x 1 mm pad at (5, 0) whose +y edge is 1 mm longer: its
    // corner (5.75, 0.5) is 0.25 mm from a track along x = 6, so 0.125 mm
    // of air; a plain 1 x 1 mm rectangle leaves 0.375 mm.
    let rules = DrcRules::new(RULE);
    let beside = vec![track(1, (6.0, -2.0), (6.0, 2.0), 2, "TopCopper")];
    let rect = board(
        vec![pad_sized((1.0, 1.0), PadShape::Rect)],
        beside.clone(),
        vec![],
    );
    assert!(run(&rect, &rules).is_clean());
    let mut trapezoid = pad_sized((1.0, 1.0), PadShape::Trapezoid);
    trapezoid.pads[0].trapezoid_delta = Point::new(1.0, 0.0);
    let report = run(&board(vec![trapezoid], beside, vec![]), &rules);
    assert_eq!(kinds(&report), [ViolationKind::Clearance], "{report:?}");
    assert_eq!(report.violations[0].measured_nm, 125_000);
}

#[test]
fn a_custom_pad_keeps_clear_of_copper_in_its_notch() {
    // #2242. An L-shaped pad, (5, 0) to (6, 1) with the corner square
    // above (5.4, 0.4) cut away. A 0.6 mm via centred in the notch is
    // 0.35 mm from both inner edges: 0.05 mm of air, a clearance finding
    // where the L's bounding box would be a short.
    let rules = DrcRules::new(RULE);
    let mut l_pad = pad_sized((1.0, 1.0), PadShape::Custom);
    l_pad.pads[0].custom_outline = [
        (0.0, 0.0),
        (1.0, 0.0),
        (1.0, 0.4),
        (0.4, 0.4),
        (0.4, 1.0),
        (0.0, 1.0),
    ]
    .map(|(x, y)| Point::new(x, y))
    .to_vec();
    l_pad.pads[0].position = Point::new(5.0, 0.0);
    let report = run(
        &board(vec![l_pad.clone()], vec![], vec![via(1, (5.75, 0.75), 2)]),
        &rules,
    );
    assert_eq!(kinds(&report), [ViolationKind::Clearance], "{report:?}");
    assert_eq!(report.violations[0].measured_nm, 50_000);

    // An outline that makes no copper is skipped, not read as a box.
    let mut broken = l_pad;
    broken.pads[0].custom_outline.truncate(2);
    let report = run(&board(vec![broken], vec![], vec![]), &rules);
    assert_eq!(report.skipped.len(), 1);
}
