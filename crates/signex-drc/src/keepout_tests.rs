//! #2234 — DRC reads the board sketch's keepouts.

use signex_types::pcb::{PadType, PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_sketch::{BoardSketch, PcbKeepout};
use signex_types::sketch::attr::KeepoutKinds;

use super::tests::{board, footprint, id, pad, track, via};
use super::*;
use pcb_geom::ItemRef;

const TOP: u32 = 1;
const KEEPOUT: u128 = 0x900;

/// A 10 x 10 mm keepout at (20, 20) on `layers`, forbidding `forbids`.
fn keep_out(mut b: PcbBoard, forbids: KeepoutKinds, layers: &[CopperLayer]) -> PcbBoard {
    let corners = [(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)];
    b.sketch = Some(BoardSketch {
        keepouts: vec![PcbKeepout {
            uuid: id(KEEPOUT),
            boundary: corners.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            layers: layers.iter().copied().map(PcbLayer::Copper).collect(),
            forbids,
        }],
        ..BoardSketch::default()
    });
    b
}

fn routing() -> KeepoutKinds {
    KeepoutKinds {
        no_routing: true,
        ..KeepoutKinds::default()
    }
}

fn keepout_hits(b: &PcbBoard) -> Vec<Violation> {
    run(b, &DrcRules::new(200_000))
        .violations
        .into_iter()
        .filter(|v| v.kind == ViolationKind::Keepout)
        .collect()
}

#[test]
fn a_track_through_a_routing_keepout_is_a_violation_on_its_layer() {
    let crossing = track(1, (15.0, 25.0), (35.0, 25.0), 0, "TopCopper");
    let b = keep_out(
        board(vec![], vec![crossing], vec![]),
        routing(),
        &[CopperLayer::Top],
    );
    let hits = keepout_hits(&b);
    assert_eq!(hits.len(), 1, "{hits:?}");
    let v = &hits[0];
    assert_eq!(v.items, vec![ItemRef::Track(id(2001))]);
    assert_eq!(v.keepout, Some(id(KEEPOUT)));
    assert_eq!(v.layers, TOP);
    assert_eq!(v.nets, (0, 0), "net 0 copper is kept out too");
}

#[test]
fn a_track_elsewhere_or_on_another_layer_passes() {
    let outside = track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper");
    let below = track(2, (15.0, 25.0), (35.0, 25.0), 1, "BottomCopper");
    let b = keep_out(
        board(vec![], vec![outside, below], vec![]),
        routing(),
        &[CopperLayer::Top],
    );
    assert!(keepout_hits(&b).is_empty());
}

#[test]
fn a_keepout_forbids_only_what_it_names() {
    let inside_via = via(1, (25.0, 25.0), 2);
    let b = keep_out(
        board(vec![], vec![], vec![inside_via.clone()]),
        routing(),
        &[CopperLayer::Top],
    );
    assert!(keepout_hits(&b).is_empty(), "a routing keepout allows vias");

    let vias = KeepoutKinds {
        no_vias: true,
        ..KeepoutKinds::default()
    };
    let b = keep_out(
        board(vec![], vec![], vec![inside_via]),
        vias,
        &[CopperLayer::Top],
    );
    let hits = keepout_hits(&b);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].items, vec![ItemRef::Via(id(3001))]);
    assert_eq!(hits[0].nets, (2, 2));
}

#[test]
fn copper_forbids_pads_tracks_and_vias_once_each() {
    let fp = footprint(1, vec![pad(11, (25.0, 25.0), 1, PadType::Smd, "TopCopper")]);
    let b = keep_out(
        board(
            vec![fp],
            vec![track(1, (15.0, 22.0), (35.0, 22.0), 1, "TopCopper")],
            vec![via(1, (28.0, 28.0), 1)],
        ),
        KeepoutKinds::ALL_COPPER,
        &[CopperLayer::Top],
    );
    let mut items: Vec<ItemRef> = keepout_hits(&b).into_iter().flat_map(|v| v.items).collect();
    items.sort_unstable();
    let mut want = vec![
        ItemRef::Pad {
            footprint: id(1001),
            pad: id(11),
        },
        ItemRef::Track(id(2001)),
        ItemRef::Via(id(3001)),
    ];
    want.sort_unstable();
    assert_eq!(
        items, want,
        "one row per item, though several kinds forbid it"
    );
}

#[test]
fn a_component_keepout_catches_a_footprint_on_its_side_only() {
    let top = footprint(1, vec![pad(11, (25.0, 25.0), 1, PadType::Smd, "TopCopper")]);
    // Local x is mirrored on the bottom, so this body also lies inside.
    let mut bottom = footprint(
        2,
        vec![pad(21, (-25.0, 25.0), 1, PadType::Smd, "BottomCopper")],
    );
    bottom.layer = "BottomCopper".into();
    let components = KeepoutKinds {
        no_components: true,
        ..KeepoutKinds::default()
    };
    let b = keep_out(
        board(vec![top, bottom], vec![], vec![]),
        components,
        &[CopperLayer::Top],
    );
    let hits = keepout_hits(&b);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].items, vec![ItemRef::Footprint(id(1001))]);
}

#[test]
fn a_drilling_keepout_catches_drilled_pads_on_any_layer() {
    let mut drilled = pad(11, (25.0, 25.0), 1, PadType::Thru, "TopCopper");
    drilled.drill = Some(signex_types::pcb::DrillDef {
        diameter: 0.5,
        shape: String::new(),
    });
    let surface = pad(12, (22.0, 22.0), 1, PadType::Smd, "TopCopper");
    let drilling = KeepoutKinds {
        no_drilling: true,
        ..KeepoutKinds::default()
    };
    let b = keep_out(
        board(vec![footprint(1, vec![drilled, surface])], vec![], vec![]),
        drilling,
        &[CopperLayer::Bottom],
    );
    let hits = keepout_hits(&b);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(
        hits[0].items,
        vec![ItemRef::Pad {
            footprint: id(1001),
            pad: id(11),
        }]
    );
}

#[test]
fn keepout_rows_sort_after_the_copper_checks() {
    let a = track(1, (15.0, 25.0), (35.0, 25.0), 1, "TopCopper");
    let b_track = track(2, (15.0, 25.0), (35.0, 25.0), 2, "TopCopper");
    let b = keep_out(
        board(vec![], vec![a, b_track], vec![]),
        routing(),
        &[CopperLayer::Top],
    );
    let report = run(&b, &DrcRules::new(200_000));
    assert_eq!(report.violations[0].kind, ViolationKind::Short);
    assert_eq!(report.count(ViolationKind::Keepout), 2);
    assert_eq!(
        report.violations.last().map(|v| v.kind),
        Some(ViolationKind::Keepout)
    );
}

#[test]
fn a_board_without_a_sketch_has_no_keepout_rows() {
    let b = board(
        vec![],
        vec![track(1, (15.0, 25.0), (35.0, 25.0), 0, "TopCopper")],
        vec![],
    );
    assert!(keepout_hits(&b).is_empty());
}

/// #2300: a keepout whose boundary does not convert is reported, not
/// dropped, so the board is not called clean.
#[test]
fn a_keepout_with_a_non_finite_corner_is_reported_as_not_checked() {
    let mut b = keep_out(
        board(vec![], vec![], vec![]),
        routing(),
        &[CopperLayer::Top],
    );
    let sketch = b.sketch.as_mut().expect("sketch");
    sketch.keepouts[0].boundary[2] = Point::new(f64::INFINITY, 30.0);
    let report = run(&b, &DrcRules::new(200_000));
    assert!(report.violations.is_empty(), "{report:?}");
    assert_eq!(report.skipped_keepouts.len(), 1, "{report:?}");
    assert_eq!(report.skipped_keepouts[0].keepout, id(KEEPOUT));
    assert!(!report.is_clean());
}

/// A keepout with no copper layer forbids nothing this check sees: a bad
/// corner on it is not reported either.
#[test]
fn a_keepout_without_copper_layers_is_neither_checked_nor_reported() {
    let mut b = keep_out(board(vec![], vec![], vec![]), routing(), &[]);
    let sketch = b.sketch.as_mut().expect("sketch");
    sketch.keepouts[0].boundary[2] = Point::new(f64::NAN, 30.0);
    let report = run(&b, &DrcRules::new(200_000));
    assert!(report.is_clean(), "{report:?}");
}

/// A courtyard graphic whose width is not a finite length.
fn broken_courtyard() -> signex_types::pcb::FpGraphic {
    signex_types::pcb::FpGraphic {
        graphic_type: "rect".into(),
        layer: "TopCourtyard".into(),
        width: f64::NAN,
        start: Some(Point::new(24.0, 24.0)),
        end: Some(Point::new(26.0, 26.0)),
        center: None,
        mid: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
        position: None,
        rotation: 0.0,
        fill: String::new(),
    }
}

/// #2300: a footprint body that does not convert, where a `no_components`
/// keepout needs it, is listed as skipped once, however many keepouts.
#[test]
fn a_body_that_does_not_convert_inside_a_component_keepout_is_skipped_once() {
    let mut fp = footprint(1, vec![pad(11, (25.0, 25.0), 1, PadType::Smd, "TopCopper")]);
    fp.graphics.push(broken_courtyard());
    let components = KeepoutKinds {
        no_components: true,
        ..KeepoutKinds::default()
    };
    let mut b = keep_out(
        board(vec![fp], vec![], vec![]),
        components,
        &[CopperLayer::Top],
    );
    let sketch = b.sketch.as_mut().expect("sketch");
    let mut second = sketch.keepouts[0].clone();
    second.uuid = id(KEEPOUT + 1);
    sketch.keepouts.push(second);
    let report = run(&b, &DrcRules::new(200_000));
    assert!(report.violations.is_empty(), "{report:?}");
    let bodies: Vec<_> = report.skipped.iter().map(|r| r.item).collect();
    assert_eq!(bodies, vec![ItemRef::Footprint(id(1001))]);
    assert!(!report.is_clean());

    // Without a component keepout the body is not needed, so not reported.
    let routing_only = keep_out(
        board(vec![b.footprints[0].clone()], vec![], vec![]),
        routing(),
        &[CopperLayer::Top],
    );
    assert!(run(&routing_only, &DrcRules::new(200_000)).is_clean());
}
