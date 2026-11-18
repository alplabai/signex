//! #2435 and #2311: the limits beyond the fixture of `signex-drc-dsl`
//! (mask expansions, unplated holes, explanations) and the fab floor.

use signex_types::pcb::{DrillDef, PadType, PcbBoard};
use signex_types::pcb_route_attrs::MaskExpansion;
use signex_types::violation::Severity;

use super::tests::{board, footprint, pad, via};
use super::*;

fn rule(name: &str, limits: Vec<Limit>) -> UserRule {
    UserRule {
        name: name.into(),
        severity: Severity::Error,
        message: None,
        nets: NetScope::Board,
        layers: None,
        pair: None,
        limits,
    }
}

fn report(b: &PcbBoard, user: Vec<UserRule>) -> DrcReport {
    let rules = DrcRules::new(200_000).with_user_rules(b, &[], user);
    run(b, &rules)
}

#[test]
fn a_manual_via_mask_expansion_outside_the_range_is_a_finding_once() {
    let mut v = via(1, (5.0, 5.0), 1);
    v.attributes.top.expansion = MaskExpansion::Manual { mm: 0.2 };
    v.attributes.bottom.expansion = MaskExpansion::Manual { mm: 0.2 };
    let b = board(vec![], vec![], vec![v.clone()]);
    let limits = vec![Limit::MaxMaskExpansion(100_000)];
    let found = report(&b, vec![rule("mask", limits.clone())]);
    let kinds: Vec<_> = found.violations.iter().map(|v| v.kind).collect();
    assert_eq!(
        kinds,
        [ViolationKind::MaskExpansion],
        "both sides alike say it once"
    );
    assert_eq!(found.violations[0].measured_nm, 200_000);
    // A tented side has no opening to check; the rule's own expansion
    // (no manual value) is not checked either.
    v.attributes.top.tented = true;
    v.attributes.bottom.expansion = MaskExpansion::Rule;
    let b = board(vec![], vec![], vec![v]);
    assert!(report(&b, vec![rule("mask", limits)]).violations.is_empty());
}

#[test]
fn an_unplated_hole_has_no_annular_ring_but_has_a_size() {
    let mut p = pad(1, (0.0, 0.0), 1, PadType::NpThru, "TopCopper");
    p.drill = Some(DrillDef {
        diameter: 0.9,
        shape: String::new(),
    });
    let b = board(vec![footprint(1, vec![p])], vec![], vec![]);
    let ring = rule("ring", vec![Limit::MinAnnularRing(500_000)]);
    assert!(report(&b, vec![ring]).violations.is_empty());
    let size = rule("size", vec![Limit::MinHoleSize(1_000_000)]);
    let found = report(&b, vec![size]);
    assert_eq!(found.count(ViolationKind::HoleSize), 1);
}

#[test]
fn a_hole_to_hole_finding_explains_both_holes() {
    let b = board(
        vec![],
        vec![],
        vec![via(1, (5.0, 5.0), 1), via(2, (5.5, 5.0), 2)],
    );
    let spacing = rule("spacing", vec![Limit::MinHoleToHole(500_000)]);
    let found = report(&b, vec![spacing]);
    let v = found
        .violations
        .iter()
        .find(|v| v.kind == ViolationKind::HoleToHole)
        .expect("a hole-to-hole finding");
    assert_eq!(v.measured_nm, 200_000);
    let why = found.rules.explain(v).expect("explained");
    assert_eq!(why.kind, LimitKind::MinHoleToHole);
    assert_eq!(why.sides.len(), 2);
}

#[test]
fn a_rule_below_the_fab_floor_is_noted_and_flagged() {
    let b = board(vec![], vec![], vec![]);
    let floor = rule("fab_clearance", vec![Limit::MinClearance(150_000)]);
    let looser = rule("tight_pitch", vec![Limit::MinClearance(100_000)]);
    let stricter = rule("safe", vec![Limit::MinClearance(300_000)]);
    let rules = DrcRules::new(200_000).with_user_rules(&b, &[], vec![floor, looser, stricter]);
    let notes: Vec<&String> = rules
        .notes
        .iter()
        .filter(|n| n.contains("fab floor"))
        .collect();
    assert_eq!(
        notes,
        [
            "DRC rule 'tight_pitch' asks min_clearance 0.1 mm, past the fab floor 'fab_clearance' of 0.15 mm"
        ]
    );
    let flags = rules.rule_flags(&b, &[]);
    let below: Vec<_> = flags
        .iter()
        .filter(|f| matches!(f.flag, RuleFlag::BelowFabFloor { .. }))
        .collect();
    assert_eq!(below.len(), 1, "{flags:?}");
    assert_eq!(below[0].rule, "tight_pitch");
    assert!(is_fab_rule("fab_clearance") && !is_fab_rule("safe"));
}

#[test]
fn every_limit_names_the_kind_of_its_finding_and_back() {
    for &limit in LimitKind::ALL {
        let kind = limit.violation();
        assert!(kind.limits().contains(&limit), "{limit:?} -> {kind:?}");
    }
    // A short is a clearance broken to 0: its limit makes clearance findings.
    for kind in ViolationKind::ALL
        .into_iter()
        .filter(|k| *k != ViolationKind::Short)
    {
        for limit in kind.limits() {
            assert_eq!(limit.violation(), kind);
        }
    }
}
