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

fn start_follows(arc: &TrackArc) -> ArcEnds {
    ArcEnds {
        arc: arc.uuid,
        index: 0,
        start: true,
        end: false,
    }
}

#[test]
fn an_arc_refuses_a_moved_end_on_its_fixed_end_or_a_chord_that_rounds_away() {
    let before = arc(1, (2.0, 0.0), (6.0, 0.0), 1, "TopCopper");
    let ends = start_follows(&before);
    assert_eq!(
        ends.checked_follow(&before, Point::new(4.0, 0.0)),
        Err(ArcFollowFault::Collapses)
    );
    // 0.8 nm of chord: the ends differ in nanometres, the middle rounds
    // onto the fixed end.
    assert_eq!(
        ends.checked_follow(&before, Point::new(4.0 - 0.8e-6, 0.0)),
        Err(ArcFollowFault::Collapses)
    );
}

#[test]
fn an_arc_may_grow_up_to_three_times_its_radius() {
    let before = arc(1, (2.0, 0.0), (6.0, 0.0), 1, "TopCopper");
    let ends = start_follows(&before);
    // A chord of 4 mm from the fixed end at x = 6.
    assert_eq!(
        ends.checked_follow(&before, Point::new(-8.4, 0.0)),
        Err(ArcFollowFault::GrowsTooMuch),
        "3.1 times"
    );
    assert!(
        ends.checked_follow(&before, Point::new(-7.6, 0.0)).is_ok(),
        "2.9 times"
    );
    let both = ArcEnds { end: true, ..ends };
    assert!(
        both.checked_follow(&before, Point::new(-80.0, 0.0)).is_ok(),
        "shifted whole, it does not grow"
    );
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

/// #2513: a dragged track takes along only the ends joined to its own ends,
/// on its layer and net, of tracks it does not carry, and none locked.
#[test]
fn only_an_end_on_an_end_of_a_dragged_track_follows() {
    let mut locked = track(7, (4.0, 0.0), (4.0, -5.0), 1, "TopCopper");
    locked.locked = true;
    let board = board(vec![
        // The dragged track, from (0, 0) to (4, 0).
        track(1, (0.0, 0.0), (4.0, 0.0), 1, "TopCopper"),
        // Joined at (4, 0), runs on to a pad end at (9, 0).
        track(2, (4.0, 0.0), (9.0, 0.0), 1, "TopCopper"),
        // Joined at (0, 0) by its end.
        track(3, (0.0, 5.0), (0.0, 0.0), 1, "TopCopper"),
        // At (4, 0) but another net.
        track(4, (4.0, 0.0), (4.0, 5.0), 2, "TopCopper"),
        // At (4, 0) but on the bottom copper.
        track(5, (4.0, 0.0), (8.0, 5.0), 1, "BottomCopper"),
        // Touching the middle of the dragged track, not an end.
        track(6, (2.0, 0.0), (2.0, 5.0), 1, "TopCopper"),
        locked,
    ]);
    let ends = tracks_on_tracks(&board, &[id(201)].into(), &BTreeSet::new());
    assert_eq!(
        ends,
        [
            TrackEnds {
                track: id(202),
                index: 1,
                start: true,
                end: false,
            },
            TrackEnds {
                track: id(203),
                index: 2,
                start: false,
                end: true,
            },
        ]
    );
    let both = tracks_on_tracks(&board, &[id(201), id(202)].into(), &BTreeSet::new());
    assert!(
        both.iter().all(|e| e.track != id(202)),
        "a track carried too is not one that follows: {both:?}"
    );
    assert!(tracks_on_tracks(&board, &BTreeSet::new(), &BTreeSet::new()).is_empty());
}

/// #2513 review fix: a joined end on a pad or via that stays is left out,
/// so that track keeps the pad or via and only the dragged track leaves.
/// When the pad's footprint or the via moves too, the end follows.
#[test]
fn a_joined_end_on_a_pad_or_via_that_stays_does_not_follow() {
    let mut board = board(vec![
        // Dragged, ending on U1 pad 1 at (9, 0).
        track(1, (5.0, 0.0), (9.0, 0.0), 1, "TopCopper"),
        // Joined to it on the pad.
        track(2, (9.0, 0.0), (9.0, 4.0), 1, "TopCopper"),
        // Dragged, ending on the via at (0, 0).
        track(3, (0.0, 0.0), (0.0, 3.0), 1, "TopCopper"),
        // Joined to it on the via.
        track(4, (-3.0, 0.0), (0.0, 0.0), 1, "TopCopper"),
    ]);
    board.vias.push(through_via(300, 0.0, 1));
    let dragged: BTreeSet<Uuid> = [id(201), id(203)].into();
    assert!(
        tracks_on_tracks(&board, &dragged, &BTreeSet::new()).is_empty(),
        "both joined ends stay on their pad and via"
    );
    let with_both = tracks_on_tracks(&board, &dragged, &[id(100), id(300)].into());
    let followers: Vec<Uuid> = with_both.iter().map(|e| e.track).collect();
    assert_eq!(followers, [id(202), id(204)], "the pad and via move too");
}

fn ends_of(segment: &Segment, start: bool) -> TrackEnds {
    TrackEnds {
        track: segment.uuid,
        index: 0,
        start,
        end: !start,
    }
}

/// Whether a track runs at 0°, 45° or 90°.
fn at_a_legal_angle(t: &Segment) -> bool {
    let (dx, dy) = ((t.end.x - t.start.x).abs(), (t.end.y - t.start.y).abs());
    dx < 1e-9 || dy < 1e-9 || (dx - dy).abs() < 1e-9
}

/// #2513, Caner (2026-11-25): a stretch off the 0°/45°/90° lines bends at
/// one 45° corner by the router's leg rule. The track keeps its fixed end
/// and the leg there; the leg to the moved end is a new track.
#[test]
fn a_stretch_off_the_legal_angles_bends_at_one_45_degree_corner() {
    let rule = stretch_rule(CornerRule::default());
    let segment = track(1, (0.0, 0.0), (5.0, 0.0), 1, "TopCopper");
    // The end moves to (5, 3): straight out of (0, 0), then 45° up.
    let up = Point::new(0.0, 3.0);
    let (kept, leg) = ends_of(&segment, false).stretch(&segment, up, rule, id(9));
    let leg = leg.expect("a corner");
    let p = Point::new;
    assert_eq!(kept.uuid, segment.uuid, "the track keeps its id");
    assert_eq!((kept.start, kept.end), (p(0.0, 0.0), p(2.0, 0.0)));
    assert_eq!(leg.uuid, id(9), "the new leg");
    assert_eq!((leg.start, leg.end), (p(2.0, 0.0), p(5.0, 3.0)));
    assert!(at_a_legal_angle(&kept) && at_a_legal_angle(&leg));
    assert_eq!(
        (leg.width, leg.net, leg.layer.as_str()),
        (0.25, 1, "TopCopper")
    );
    // The start moving instead: the end stays, the corner is on its side.
    let (kept, leg) = ends_of(&segment, true).stretch(&segment, up, rule, id(9));
    let leg = leg.expect("a corner");
    assert_eq!((kept.start, kept.end), (p(3.0, 0.0), p(5.0, 0.0)));
    assert_eq!((leg.start, leg.end), (p(0.0, 3.0), p(3.0, 0.0)));
    assert!(at_a_legal_angle(&kept) && at_a_legal_angle(&leg));
}

/// A stretch that stays on an axis or on an exact 45° line stays one track.
#[test]
fn a_stretch_on_a_legal_angle_stays_one_track() {
    let rule = stretch_rule(CornerRule::default());
    let segment = track(1, (0.0, 0.0), (5.0, 0.0), 1, "TopCopper");
    let ends = ends_of(&segment, false);
    let cases = [
        // Along the axis, onto an exact 45° line, onto the vertical.
        ((2.0, 0.0), (7.0, 0.0)),
        ((0.0, 5.0), (5.0, 5.0)),
        ((-5.0, 4.0), (0.0, 4.0)),
    ];
    for (delta, to) in cases {
        let by = Point::new(delta.0, delta.1);
        let (kept, leg) = ends.stretch(&segment, by, rule, id(9));
        assert!(leg.is_none(), "{delta:?}: no corner");
        assert_eq!(kept.start, Point::new(0.0, 0.0), "{delta:?}");
        assert_eq!(kept.end, Point::new(to.0, to.1), "{delta:?}");
    }
}

/// The router's rule is kept when it bends at 45°; a 90° or any-angle
/// router rule still gives a 45° corner.
#[test]
fn the_stretch_takes_the_routers_45_degree_rule() {
    use signex_types::corner::CornerMode;
    let diagonal_first = CornerRule {
        mode: CornerMode::Angle45Start,
        is_flipped: false,
    };
    assert_eq!(stretch_rule(diagonal_first), diagonal_first);
    for mode in [CornerMode::Ortho90, CornerMode::FreeAngle] {
        let live = CornerRule {
            mode,
            is_flipped: true,
        };
        assert_eq!(stretch_rule(live).mode, CornerMode::Angle45End, "{mode:?}");
    }
    let segment = track(1, (0.0, 0.0), (5.0, 0.0), 1, "TopCopper");
    let up = Point::new(0.0, 3.0);
    let (kept, leg) = ends_of(&segment, false).stretch(&segment, up, diagonal_first, id(9));
    // 45° out of (0, 0) first, then straight on to (5, 3).
    assert_eq!(kept.end, Point::new(3.0, 3.0));
    let leg = leg.map(|l| (l.start, l.end));
    assert_eq!(leg, Some((Point::new(3.0, 3.0), Point::new(5.0, 3.0))));
}
