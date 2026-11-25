//! #2515: tracks, pads and drawings of no net are copper the check measures
//! against every net's copper and the board edge; they never connect.

use pcb_geom::ItemRef;
use signex_types::pcb::{BoardText, PadType, PcbBoard, Point};
use signex_types::violation::Severity;

use super::tests::{board, footprint, id, pad, track};
use super::*;

const MM: i64 = 1_000_000;

fn no_net_pad(n: u128, at: (f64, f64)) -> signex_types::pcb::Pad {
    let mut p = pad(n, at, 0, PadType::Smd, "TopCopper");
    p.net = None;
    p
}

fn pad_ref(n: u128) -> ItemRef {
    ItemRef::Pad {
        footprint: id(1001),
        pad: id(n),
    }
}

fn findings_of(report: &DrcReport, item: ItemRef) -> Vec<&Violation> {
    report
        .violations
        .iter()
        .filter(|v| v.items.contains(&item))
        .collect()
}

#[test]
fn a_track_of_no_net_too_near_a_netted_track_is_a_clearance_violation() {
    let b = board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (0.0, 0.5), (10.0, 0.5), 0, "TopCopper"),
        ],
        vec![],
    );
    let report = run(&b, &DrcRules::new(MM));
    let found = findings_of(&report, ItemRef::Track(id(2002)));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].kind, ViolationKind::Clearance);
    assert_eq!(
        found[0].items,
        [ItemRef::Track(id(2001)), ItemRef::Track(id(2002))]
    );
    assert_eq!(found[0].nets, (1, 0));
    assert_eq!(found[0].measured_nm, 250_000);

    let far = board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (0.0, 0.5), (10.0, 0.5), 0, "BottomCopper"),
        ],
        vec![],
    );
    assert!(
        run(&far, &DrcRules::new(MM)).violations.is_empty(),
        "another layer"
    );
}

#[test]
fn a_pad_of_no_net_on_a_netted_track_is_a_short() {
    let b = board(
        vec![footprint(1, vec![no_net_pad(7, (5.0, 0.0))])],
        vec![track(1, (0.0, 0.0), (10.0, 0.0), 2, "TopCopper")],
        vec![],
    );
    let report = run(&b, &DrcRules::new(200_000));
    let found = findings_of(&report, pad_ref(7));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].kind, ViolationKind::Short);
    assert_eq!(found[0].nets, (0, 2));
}

#[test]
fn two_items_of_no_net_are_never_a_pair() {
    let b = board(
        vec![footprint(1, vec![no_net_pad(7, (5.0, 0.0))])],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 0, "TopCopper"),
            track(2, (0.0, 0.3), (10.0, 0.3), 0, "TopCopper"),
        ],
        vec![],
    );
    let report = run(&b, &DrcRules::new(MM));
    assert!(report.violations.is_empty(), "{:?}", report.violations);
}

/// The ratsnest reads netted copper only: a track of no net across two
/// pads of one net shorts to each but does not join them.
#[test]
fn copper_of_no_net_never_makes_a_connection() {
    let pads = vec![
        pad(1, (0.0, 0.0), 1, PadType::Smd, "TopCopper"),
        pad(2, (10.0, 0.0), 1, PadType::Smd, "TopCopper"),
    ];
    let open = board(vec![footprint(1, pads.clone())], vec![], vec![]);
    let rules = DrcRules::new(200_000);
    assert_eq!(run(&open, &rules).count(ViolationKind::Unconnected), 1);

    let bridged = board(
        vec![footprint(1, pads)],
        vec![track(1, (0.0, 0.0), (10.0, 0.0), 0, "TopCopper")],
        vec![],
    );
    let report = run(&bridged, &rules);
    assert_eq!(report.count(ViolationKind::Unconnected), 1, "still open");
    assert_eq!(report.count(ViolationKind::Short), 2, "one per pad");
}

fn square_outline(b: &mut PcbBoard) {
    b.outline = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]
        .iter()
        .map(|&(x, y)| Point::new(x, y))
        .collect();
}

fn edge_rules(b: &PcbBoard) -> DrcRules {
    let edge = UserRule {
        name: "edge".into(),
        severity: Severity::Error,
        message: None,
        nets: NetScope::Board,
        layers: None,
        pair: None,
        limits: vec![Limit::MinEdgeClearance(MM)],
    };
    DrcRules::new(200_000).with_user_rules(b, &[], vec![edge])
}

#[test]
fn copper_of_no_net_keeps_the_edge_clearance() {
    let mut b = board(
        vec![footprint(1, vec![no_net_pad(7, (10.0, 19.0))])],
        vec![track(2, (2.0, 0.5), (10.0, 0.5), 0, "TopCopper")],
        vec![],
    );
    square_outline(&mut b);
    b.texts.push(BoardText {
        uuid: id(50),
        text: "A1".into(),
        position: Point::new(19.5, 10.0),
        layer: "TopCopper".into(),
        font_size: 1.0,
        rotation: 0.0,
    });
    let report = run(&b, &edge_rules(&b));
    let edge = |item: ItemRef| {
        report
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::EdgeClearance && v.items == [item])
            .collect::<Vec<_>>()
    };
    let track_hits = edge(ItemRef::Track(id(2002)));
    assert_eq!(track_hits.len(), 1, "{:?}", report.violations);
    // A 0.25 mm track on y = 0.5: its copper is 0.375 mm off the edge.
    assert_eq!(track_hits[0].measured_nm, 375_000);
    assert_eq!(track_hits[0].nets, (0, 0));
    // The 1 mm pad on y = 19 reaches y = 19.5, 0.5 mm off the edge.
    assert_eq!(edge(pad_ref(7)).len(), 1, "the pad of no net");
    assert_eq!(edge(ItemRef::Text(id(50))).len(), 1, "copper text");
    assert_eq!(report.count(ViolationKind::EdgeClearance), 3);
}
