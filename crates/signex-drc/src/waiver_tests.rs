//! #2399: the waiver key names one finding the same way in every run and
//! wherever the board lives, and not its neighbour. #2307: the geometry
//! fingerprint lapses a waiver whose objects changed, and only then.

use pcb_geom::{PickKind, PointNm};
use serde_json::json;
use signex_types::pcb::{
    BoardGraphic, BoardText, FpGraphic, NetDef, PadType, PcbBoard, Point, Zone,
};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_options::DrcWaiver;
use signex_types::pcb_sketch::{BoardSketch, PcbKeepout};
use signex_types::pcb_stack::LayerSizes;
use signex_types::sketch::attr::KeepoutKinds;

use super::*;
use crate::tests::{board, footprint, id, pad, track, via};
use crate::{DrcRules, ViolationKind, run};

/// Tracks of nets 1 and 2 on top copper, `pitch` mm apart, from `x`.
fn pair(x: f64, pitch: f64) -> PcbBoard {
    board(
        vec![],
        vec![
            track(1, (x, 0.0), (x + 10.0, 0.0), 1, "TopCopper"),
            track(2, (x, pitch), (x + 10.0, pitch), 2, "TopCopper"),
        ],
        vec![],
    )
}

fn only_key(b: &PcbBoard) -> String {
    let report = run(b, &DrcRules::new(200_000));
    assert_eq!(report.violations.len(), 1, "{report:?}");
    waiver_key(&report.violations[0])
}

fn finding(items: Vec<ItemRef>, keepout: Option<uuid::Uuid>) -> Violation {
    Violation {
        kind: ViolationKind::Keepout,
        items,
        nets: (0, 0),
        layers: 1,
        at: PointNm::new(0, 0),
        measured_nm: 0,
        required_nm: 0,
        keepout,
        rule: None,
    }
}

fn waiver(v: &Violation, fingerprint: &str, note: &str) -> DrcWaiver {
    DrcWaiver {
        key: waiver_key(v),
        rule: rule_id(v).into(),
        note: note.into(),
        fingerprint: fingerprint.into(),
    }
}

/// A board with one of every free object: a zone, a string, an arc, a
/// free pad, a dimension, a drawing and a keepout, ids 0x50..0x56.
fn everything() -> PcbBoard {
    let fp = footprint(1, vec![pad(5, (1.0, 0.0), 1, PadType::Smd, "TopCopper")]);
    let mut b = board(
        vec![fp],
        vec![track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper")],
        vec![via(1, (5.0, 5.0), 2)],
    );
    b.nets = vec![
        NetDef {
            number: 1,
            name: "GND".into(),
        },
        NetDef {
            number: 2,
            name: "VCC".into(),
        },
    ];
    let point = |x: f64, y: f64| json!({"x": x, "y": y});
    macro_rules! from {
        ($value:expr) => {
            serde_json::from_value($value).expect("test object")
        };
    }
    let zone: Zone = from!(json!({"uuid": id(0x50), "net": 1, "layer": "TopCopper",
        "outline": [point(0.0, 0.0), point(4.0, 0.0), point(4.0, 4.0)]}));
    b.zones.push(zone);
    let text: BoardText = from!(json!({"uuid": id(0x51), "text": "REV A",
        "position": point(2.0, 2.0), "layer": "TopSilk", "font_size": 1.0}));
    b.texts.push(text);
    let arc: TrackArc = from!(json!({"uuid": id(0x52), "start": point(0.0, 0.0),
        "mid": point(1.0, 1.0), "end": point(2.0, 0.0), "width": 0.2,
        "layer": "TopCopper", "net": 2}));
    b.free.arcs.push(arc);
    b.free
        .pads
        .push(pad(0x53, (7.0, 7.0), 2, PadType::Smd, "TopCopper"));
    b.free.dimensions.push(LinearDimension::new(
        id(0x54),
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        2.0,
    ));
    let drawing: BoardGraphic = from!(json!({"uuid": id(0x55), "graphic_type": "line",
        "layer": "TopSilk", "width": 0.15, "start": point(0.0, 0.0), "end": point(3.0, 0.0)}));
    b.graphics.push(drawing);
    b.sketch = Some(BoardSketch {
        keepouts: vec![PcbKeepout {
            uuid: id(0x56),
            boundary: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
            ],
            layers: vec![PcbLayer::Copper(CopperLayer::Top)],
            forbids: KeepoutKinds::ALL_COPPER,
        }],
        ..BoardSketch::default()
    });
    b
}

/// `edit` of `everything()` changes the fingerprint of a finding naming
/// `items`/`keepout`; an untouched copy keeps it.
fn assert_edit_lapses(items: Vec<ItemRef>, keepout: Option<uuid::Uuid>, edit: fn(&mut PcbBoard)) {
    let v = finding(items, keepout);
    let before = fingerprint(&v, &everything());
    assert_eq!(before.len(), 32);
    assert_eq!(fingerprint(&v, &everything()), before, "unchanged board");
    let mut edited = everything();
    edit(&mut edited);
    assert_ne!(fingerprint(&v, &edited), before, "{v:?}");
}

#[test]
fn moving_the_items_keeps_the_key() {
    let here = only_key(&pair(0.0, 0.3));
    let moved = only_key(&pair(25.0, 0.35));
    assert_eq!(here, moved);
    assert_eq!(here.len(), 32);
}

#[test]
fn a_finding_of_other_items_has_another_key() {
    let mut other = pair(0.0, 0.3);
    other.segments[1].uuid = uuid::Uuid::from_u128(0x99);
    assert_ne!(only_key(&pair(0.0, 0.3)), only_key(&other));
}

#[test]
fn a_finding_is_matched_to_its_waiver_and_filed_under_its_rule() {
    let b = pair(0.0, 0.3);
    let report = run(&b, &DrcRules::new(200_000));
    let v = &report.violations[0];
    assert_eq!(v.kind, ViolationKind::Clearance);
    assert_eq!(rule_id(v), "builtin::clearance");
    let waivers = vec![
        DrcWaiver {
            key: "another".into(),
            rule: "builtin::short".into(),
            note: String::new(),
            fingerprint: String::new(),
        },
        waiver(v, &fingerprint(v, &b), "accepted"),
    ];
    match waiver_state(&waivers, v, &b) {
        WaiverState::Waived(w) => assert_eq!(w.note, "accepted"),
        other => panic!("{other:?}"),
    }
    assert_eq!(waiver_state(&waivers[..1], v, &b), WaiverState::Open);
}

/// A link with no pad at either end has no object to name: two of them
/// at different places are told apart by where they are.
#[test]
fn findings_that_name_no_item_are_named_by_where_they_are() {
    let link = |x: i64| crate::Violation {
        kind: ViolationKind::Unconnected,
        items: Vec::new(),
        nets: (1, 1),
        layers: 0,
        at: pcb_geom::PointNm::new(x, 0),
        measured_nm: 1_000_000,
        required_nm: 0,
        keepout: None,
        rule: None,
    };
    assert_ne!(waiver_key(&link(0)), waiver_key(&link(5)));
    assert_eq!(waiver_key(&link(0)), waiver_key(&link(0)));
}

#[test]
fn a_track_fingerprint_follows_its_geometry_and_net() {
    let t = vec![ItemRef::Track(id(2001))];
    assert_edit_lapses(t.clone(), None, |b| b.segments[0].end.y += 0.05);
    assert_edit_lapses(t.clone(), None, |b| b.segments[0].width = 0.3);
    assert_edit_lapses(t.clone(), None, |b| {
        b.segments[0].layer = "BottomCopper".into()
    });
    assert_edit_lapses(t, None, |b| b.nets[0].name = "AGND".into());
}

#[test]
fn a_via_fingerprint_follows_its_size_and_span() {
    let v = vec![ItemRef::Via(id(3001))];
    assert_edit_lapses(v.clone(), None, |b| b.vias[0].position.x += 0.05);
    assert_edit_lapses(v.clone(), None, |b| b.vias[0].diameter = 0.7);
    assert_edit_lapses(v, None, |b| {
        b.vias[0].via_type = signex_types::pcb::ViaType::Blind;
        b.vias[0].layers = vec!["TopCopper".into(), "Inner1Copper".into()];
    });
}

/// #2401: DRC measures a via per layer, so an inner layer's diameter is
/// part of its fingerprint.
#[test]
fn a_via_inner_layer_diameter_changes_the_fingerprint() {
    let sized = |inner: f64| LayerSizes::TopMiddleBottom {
        top: 0.6,
        middle: inner,
        bottom: 0.6,
    };
    let v = finding(vec![ItemRef::Via(id(3001))], None);
    let mut b = everything();
    b.vias[0].layer_sizes = Some(sized(0.6));
    let before = fingerprint(&v, &b);
    b.vias[0].layer_sizes = Some(sized(0.9));
    assert_ne!(fingerprint(&v, &b), before);
}

/// #2401: the same for a pad's per-layer size.
#[test]
fn a_pad_inner_layer_size_changes_the_fingerprint() {
    let sized = |inner: f64| LayerSizes::TopMiddleBottom {
        top: Point::new(1.0, 1.0),
        middle: Point::new(inner, 1.0),
        bottom: Point::new(1.0, 1.0),
    };
    let v = finding(
        vec![ItemRef::Pad {
            footprint: id(1001),
            pad: id(5),
        }],
        None,
    );
    let mut b = everything();
    b.footprints[0].pads[0].layer_sizes = Some(sized(1.0));
    let before = fingerprint(&v, &b);
    b.footprints[0].pads[0].layer_sizes = Some(sized(1.4));
    assert_ne!(fingerprint(&v, &b), before);
}

#[test]
fn a_pad_fingerprint_follows_its_footprint_and_its_own_geometry() {
    let p = vec![ItemRef::Pad {
        footprint: id(1001),
        pad: id(5),
    }];
    assert_edit_lapses(p.clone(), None, |b| b.footprints[0].position.x += 0.05);
    assert_edit_lapses(p.clone(), None, |b| b.footprints[0].rotation = 90.0);
    assert_edit_lapses(p.clone(), None, |b| {
        b.footprints[0].pads[0].size = Point::new(2.0, 1.0);
    });
    assert_edit_lapses(p, None, |b| b.footprints[0].pads[0].position.y += 0.05);
}

#[test]
fn a_footprint_fingerprint_follows_its_placement_and_body() {
    let f = vec![ItemRef::Footprint(id(1001))];
    assert_edit_lapses(f.clone(), None, |b| b.footprints[0].rotation = 45.0);
    assert_edit_lapses(f.clone(), None, |b| {
        b.footprints[0].layer = "BottomCopper".into()
    });
    assert_edit_lapses(f.clone(), None, |b| {
        b.footprints[0].pads[0].position.x += 0.05;
    });
    assert_edit_lapses(f, None, |b| {
        let courtyard: FpGraphic = serde_json::from_value(json!({"graphic_type": "line",
            "layer": "TopCourtyard", "width": 0.05,
            "start": {"x": -2.0, "y": -2.0}, "end": {"x": 2.0, "y": 2.0}}))
        .expect("courtyard");
        b.footprints[0].graphics.push(courtyard);
    });
}

/// A footprint graphic DRC never checks (silkscreen) keeps the fingerprint.
#[test]
fn a_silkscreen_edit_keeps_the_footprint_fingerprint() {
    let v = finding(vec![ItemRef::Footprint(id(1001))], None);
    let before = fingerprint(&v, &everything());
    let mut b = everything();
    let silk: FpGraphic = serde_json::from_value(json!({"graphic_type": "line",
        "layer": "TopSilkscreen", "width": 0.15,
        "start": {"x": -1.0, "y": 0.0}, "end": {"x": 1.0, "y": 0.0}}))
    .expect("silk");
    b.footprints[0].graphics.push(silk);
    assert_eq!(fingerprint(&v, &b), before);
}

/// The fingerprint reads the stored turn, not a body that went through
/// sin/cos, so a turned footprint keeps it across a save and a load.
#[test]
fn a_turned_footprint_keeps_its_fingerprint_across_a_save() {
    let v = finding(vec![ItemRef::Footprint(id(1001))], None);
    let mut b = everything();
    b.footprints[0].rotation = 33.3;
    b.footprints[0].position = Point::new(0.1 + 0.2, 7.7);
    let text = signex_types::format::SnxPcb::new(b.clone())
        .write_string()
        .expect("writes");
    let back = signex_types::format::SnxPcb::parse(&text)
        .expect("reads")
        .board;
    assert_eq!(fingerprint(&v, &back), fingerprint(&v, &b));
}

#[test]
fn a_zone_fingerprint_follows_its_outline_and_net() {
    let z = vec![ItemRef::Zone {
        zone: id(0x50),
        kind: PickKind::Polygons,
    }];
    assert_edit_lapses(z.clone(), None, |b| b.zones[0].outline[2].x += 0.05);
    assert_edit_lapses(z, None, |b| b.zones[0].net = 2);
}

#[test]
fn a_string_fingerprint_follows_its_place() {
    let t = vec![ItemRef::Text(id(0x51))];
    assert_edit_lapses(t.clone(), None, |b| b.texts[0].position.x += 0.05);
    assert_edit_lapses(t, None, |b| b.texts[0].rotation = 90.0);
}

#[test]
fn an_arc_fingerprint_follows_its_geometry_and_net() {
    let a = vec![ItemRef::Arc(id(0x52))];
    assert_edit_lapses(a.clone(), None, |b| b.free.arcs[0].mid.y += 0.05);
    assert_edit_lapses(a, None, |b| b.nets[1].name = "VBUS".into());
}

#[test]
fn a_free_pad_fingerprint_follows_its_geometry() {
    let p = vec![ItemRef::FreePad(id(0x53))];
    assert_edit_lapses(p.clone(), None, |b| b.free.pads[0].position.x += 0.05);
    assert_edit_lapses(p, None, |b| b.free.pads[0].size = Point::new(1.0, 2.0));
}

#[test]
fn a_dimension_fingerprint_follows_its_geometry() {
    let d = vec![ItemRef::Dimension(id(0x54))];
    assert_edit_lapses(d, None, |b| b.free.dimensions[0].offset = 3.0);
}

#[test]
fn a_drawing_fingerprint_follows_its_geometry() {
    let g = vec![ItemRef::Graphic {
        graphic: id(0x55),
        kind: PickKind::Other,
    }];
    assert_edit_lapses(g, None, |b| b.graphics[0].width = 0.3);
}

#[test]
fn a_keepout_fingerprint_follows_its_boundary() {
    assert_edit_lapses(vec![ItemRef::Track(id(2001))], Some(id(0x56)), |b| {
        let sketch = b.sketch.as_mut().expect("sketch");
        sketch.keepouts[0].boundary[1].x += 0.05;
    });
}

/// An item the board lacks hashes a fixed marker, so the fingerprint is
/// stable and differs from the item's own.
#[test]
fn a_missing_item_has_a_stable_fingerprint_of_its_own() {
    let v = finding(vec![ItemRef::Track(id(2001))], None);
    let mut gone = everything();
    gone.segments.clear();
    assert_eq!(fingerprint(&v, &gone), fingerprint(&v, &gone.clone()));
    assert_ne!(fingerprint(&v, &gone), fingerprint(&v, &everything()));
}

#[test]
fn an_unrelated_edit_keeps_the_fingerprint() {
    let v = finding(vec![ItemRef::Track(id(2001))], None);
    let mut b = everything();
    b.vias[0].position.x += 3.0;
    b.segments[0].locked = true;
    assert_eq!(fingerprint(&v, &b), fingerprint(&v, &everything()));
}

#[test]
fn renumbering_the_nets_keeps_the_fingerprint() {
    let v = finding(vec![ItemRef::Track(id(2001)), ItemRef::Via(id(3001))], None);
    let before = fingerprint(&v, &everything());
    let mut b = everything();
    b.nets[0].number = 7;
    b.segments[0].net = 7;
    b.nets[1].number = 9;
    b.vias[0].net = 9;
    assert_eq!(fingerprint(&v, &b), before);
}

fn clearance() -> (PcbBoard, Violation) {
    let b = pair(0.0, 0.3);
    let v = run(&b, &DrcRules::new(200_000)).violations[0].clone();
    (b, v)
}

/// `b` with the second track 0.05 mm further off: the same finding, worse
/// or better, on changed geometry.
fn moved(b: &PcbBoard) -> PcbBoard {
    let mut b = b.clone();
    b.segments[1].start.y += 0.05;
    b.segments[1].end.y += 0.05;
    b
}

#[test]
fn a_waiver_holds_while_the_geometry_matches_and_lapses_after_a_move() {
    let (b, v) = clearance();
    let waivers = vec![waiver(&v, &fingerprint(&v, &b), "ok")];
    assert_eq!(
        waiver_state(&waivers, &v, &b),
        WaiverState::Waived(&waivers[0])
    );
    assert_eq!(
        waiver_state(&waivers, &v, &moved(&b)),
        WaiverState::Lapsed(&waivers[0])
    );
}

/// A waiver made before #2307 has no fingerprint: it lapses once, even on
/// unchanged objects, and is waived again.
#[test]
fn a_waiver_without_a_fingerprint_lapses() {
    let (b, v) = clearance();
    let waivers = vec![waiver(&v, "", "old")];
    assert_eq!(
        waiver_state(&waivers, &v, &b),
        WaiverState::Lapsed(&waivers[0])
    );
}

/// With a waiver without a fingerprint in the list, a current one still
/// wins, and with none current the last same-key waiver lapses.
#[test]
fn a_waiver_without_a_fingerprint_never_wins_in_a_mixed_list() {
    let (b, v) = clearance();
    let after = moved(&b);
    let stale = [
        waiver(&v, "", "old"),
        waiver(&v, &fingerprint(&v, &b), "before the move"),
    ];
    assert_eq!(
        waiver_state(&stale, &v, &after),
        WaiverState::Lapsed(&stale[1])
    );
    let current = [
        waiver(&v, "", "old"),
        waiver(&v, &fingerprint(&v, &after), "now"),
    ];
    assert_eq!(
        waiver_state(&current, &v, &after),
        WaiverState::Waived(&current[1])
    );
}

/// Review point 4: an older stale waiver never shadows a newer current
/// one with the same key, in either order.
#[test]
fn a_current_waiver_wins_over_a_stale_one_with_the_same_key() {
    let (b, v) = clearance();
    let stale = waiver(&v, &fingerprint(&v, &moved(&b)), "stale");
    let current = waiver(&v, &fingerprint(&v, &b), "current");
    for waivers in [
        vec![stale.clone(), current.clone()],
        vec![current.clone(), stale.clone()],
    ] {
        match waiver_state(&waivers, &v, &b) {
            WaiverState::Waived(w) => assert_eq!(w.note, "current"),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn when_every_waiver_is_stale_the_last_one_lapses() {
    let (b, v) = clearance();
    let first = waiver(&v, "0000", "first");
    let last = waiver(&v, "1111", "last");
    let waivers = [first, last];
    assert_eq!(
        waiver_state(&waivers, &v, &b),
        WaiverState::Lapsed(&waivers[1])
    );
}

#[test]
fn a_waiver_json_names_its_rule_key_and_fingerprint() {
    let (b, v) = clearance();
    let w = waiver(&v, &fingerprint(&v, &b), "accepted");
    assert_eq!(
        waiver_json(&w),
        json!({"kind": "finding", "rule_id": "builtin::clearance", "key": w.key,
            "fingerprint": w.fingerprint, "reason": "accepted"})
    );
}
