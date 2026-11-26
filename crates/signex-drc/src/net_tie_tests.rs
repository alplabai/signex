//! A net tie's copper bridge (#1277; Caner, 2026-11-25): the pads of one
//! net-tie footprint may short their nets; any other copper may not.

use super::tests::{board, footprint, pad, track};
use super::*;
use signex_types::pcb::{ComponentType, Footprint, PadType};

const RULE: i64 = 200_000;

/// Two 1 mm pads on nets 1 and 2 whose centres are `pitch_mm` apart, in a
/// footprint of type `kind`.
fn two_pads(n: u128, x: f64, pitch_mm: f64, kind: ComponentType) -> Footprint {
    let mut fp = footprint(
        n,
        vec![
            pad(10 * n + 1, (x, 0.0), 1, PadType::Smd, "TopCopper"),
            pad(
                10 * n + 2,
                (x + pitch_mm, 0.0),
                2,
                PadType::Smd,
                "TopCopper",
            ),
        ],
    );
    fp.component.component_type = kind;
    fp
}

fn kinds(report: &DrcReport) -> Vec<ViolationKind> {
    report.violations.iter().map(|v| v.kind).collect()
}

#[test]
fn overlapping_pads_of_a_standard_part_are_a_short() {
    let b = board(
        vec![two_pads(1, 0.0, 0.8, ComponentType::Standard)],
        vec![],
        vec![],
    );
    assert_eq!(
        kinds(&run(&b, &DrcRules::new(RULE))),
        [ViolationKind::Short]
    );
}

#[test]
fn the_pads_of_a_net_tie_bridge_their_nets_without_a_finding() {
    for kind in [ComponentType::NetTie, ComponentType::NetTieInBom] {
        let b = board(vec![two_pads(1, 0.0, 0.8, kind)], vec![], vec![]);
        assert!(run(&b, &DrcRules::new(RULE)).is_clean(), "{kind:?}");
    }
}

#[test]
fn the_pads_of_a_net_tie_may_sit_closer_than_the_clearance() {
    let standard = board(
        vec![two_pads(1, 0.0, 1.1, ComponentType::Standard)],
        vec![],
        vec![],
    );
    assert_eq!(
        kinds(&run(&standard, &DrcRules::new(RULE))),
        [ViolationKind::Clearance]
    );
    let tie = board(
        vec![two_pads(1, 0.0, 1.1, ComponentType::NetTie)],
        vec![],
        vec![],
    );
    assert!(run(&tie, &DrcRules::new(RULE)).is_clean());
}

/// The exemption is the tie's own pads: a track of the other net over a
/// tie pad is a short as on any part.
#[test]
fn other_copper_against_a_tie_pad_is_still_checked() {
    let tie = two_pads(1, 0.0, 0.8, ComponentType::NetTie);
    let over_pad_one = track(1, (-1.0, 0.0), (0.2, 0.0), 2, "TopCopper");
    let b = board(vec![tie], vec![over_pad_one], vec![]);
    let report = run(&b, &DrcRules::new(RULE));
    let shorts: Vec<&Violation> = report
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Short)
        .collect();
    assert_eq!(shorts.len(), 1, "{:?}", report.violations);
    assert!(
        shorts[0]
            .items
            .contains(&pcb_geom::ItemRef::Track(super::tests::id(2001)))
    );
}

/// Two ties side by side: a pad of one against a pad of the other is not a
/// bridge of either.
#[test]
fn pads_of_two_different_ties_are_checked() {
    let mut left = two_pads(1, 0.0, 0.8, ComponentType::NetTie);
    let right = two_pads(2, 1.6, 0.8, ComponentType::NetTie);
    // The left tie's pad 2 (net 2) overlaps the right tie's pad 1 (net 1).
    left.pads[1].position.x = 1.0;
    let b = board(vec![left, right], vec![], vec![]);
    let report = run(&b, &DrcRules::new(RULE));
    let shorts: Vec<&Violation> = report
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Short)
        .collect();
    assert_eq!(shorts.len(), 1, "{:?}", report.violations);
    let pads = |fp: u128, pad: u128| pcb_geom::ItemRef::Pad {
        footprint: super::tests::id(1000 + fp),
        pad: super::tests::id(pad),
    };
    assert_eq!(shorts[0].items, [pads(1, 12), pads(2, 21)]);
}
