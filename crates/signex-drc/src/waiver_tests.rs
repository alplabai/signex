//! #2399: the waiver key names one finding the same way in every run and
//! wherever the board lives, and not its neighbour.

use signex_types::pcb_options::DrcWaiver;

use super::*;
use crate::tests::{board, track};
use crate::{DrcRules, ViolationKind, run};

/// Tracks of nets 1 and 2 on top copper, `pitch` mm apart, from `x`.
fn pair(x: f64, pitch: f64) -> signex_types::pcb::PcbBoard {
    board(
        vec![],
        vec![
            track(1, (x, 0.0), (x + 10.0, 0.0), 1, "TopCopper"),
            track(2, (x, pitch), (x + 10.0, pitch), 2, "TopCopper"),
        ],
        vec![],
    )
}

fn only_key(b: &signex_types::pcb::PcbBoard) -> String {
    let report = run(b, &DrcRules::new(200_000));
    assert_eq!(report.violations.len(), 1, "{report:?}");
    waiver_key(&report.violations[0])
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
    let report = run(&pair(0.0, 0.3), &DrcRules::new(200_000));
    let v = &report.violations[0];
    assert_eq!(v.kind, ViolationKind::Clearance);
    assert_eq!(rule_id(v), "builtin::clearance");
    let waivers = vec![
        DrcWaiver {
            key: "another".into(),
            rule: "builtin::short".into(),
            note: String::new(),
        },
        DrcWaiver {
            key: waiver_key(v),
            rule: rule_id(v).into(),
            note: "accepted".into(),
        },
    ];
    assert_eq!(
        waiver_of(&waivers, v).map(|w| w.note.as_str()),
        Some("accepted")
    );
    assert!(waiver_of(&waivers[..1], v).is_none());
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
