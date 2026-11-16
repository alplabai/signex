//! #2398 - class-pair rules (the class-to-class clearance matrix): a cell
//! governs a clearance between a net of one class and a net of the other,
//! both sides, and nothing else; a rule for one net still wins over it.

use signex_types::net_class::NetClass;
use signex_types::pcb::{NetDef, PcbBoard};
use signex_types::violation::Severity;

use super::tests::{board, track};
use super::*;

/// Nets 1 `HV_A` (class `HV`, 0.5 mm), 2 `LV_A` (class `LV`), 3 `SIG`.
fn classes() -> Vec<NetClass> {
    let mut hv = NetClass::named("HV");
    hv.nets = vec!["HV_A".into()];
    hv.clearance = 0.5;
    let mut lv = NetClass::named("LV");
    lv.nets = vec!["LV_A".into()];
    vec![hv, lv]
}

/// A track of net 1 and a track of `other`, 0.4 mm apart centre to
/// centre (0.25 mm wide, so a 0.15 mm gap).
fn facing(other: u32) -> PcbBoard {
    let mut b = board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (0.0, 0.4), (10.0, 0.4), other, "TopCopper"),
        ],
        vec![],
    );
    b.nets = [(1, "HV_A"), (2, "LV_A"), (3, "SIG")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    b
}

fn rule(name: &str, nets: NetScope, nm: i64) -> UserRule {
    UserRule {
        name: name.into(),
        severity: Severity::Error,
        message: None,
        nets,
        layers: None,
        pair: None,
        limits: vec![Limit::MinClearance(nm)],
    }
}

fn cell(nm: i64) -> UserRule {
    rule("hv_lv", NetScope::ClassPair("HV".into(), "LV".into()), nm)
}

fn rules_for(b: &PcbBoard, user: Vec<UserRule>) -> DrcRules {
    let mut rules = DrcRules::for_board(b, &classes()).unwrap();
    rules.clearance = 200_000;
    rules.from_default = false;
    rules.with_user_rules(b, &classes(), user)
}

fn verdict(b: &PcbBoard, user: Vec<UserRule>) -> Option<(i64, Option<String>)> {
    let report = run(b, &rules_for(b, user));
    report
        .violations
        .first()
        .map(|v| (v.required_nm, v.rule.clone()))
}

#[test]
fn a_cell_governs_both_sides_of_its_two_classes() {
    // Without the cell HV_A asks its class's 0.5 mm of LV_A.
    assert_eq!(verdict(&facing(2), vec![]), Some((500_000, None)));
    // The cell asks 0.3 mm of every HV-LV pair, on both sides.
    let want = Some((300_000, Some("user::hv_lv".to_string())));
    assert_eq!(verdict(&facing(2), vec![cell(300_000)]), want);
    // The other order of the classes is the same cell.
    let flipped = rule(
        "hv_lv",
        NetScope::ClassPair("LV".into(), "HV".into()),
        300_000,
    );
    assert_eq!(verdict(&facing(2), vec![flipped]), want);
}

#[test]
fn a_cell_does_not_touch_other_pairs() {
    // HV_A against SIG (no class): the cell is for LV only.
    assert_eq!(
        verdict(&facing(3), vec![cell(300_000)]),
        Some((500_000, None))
    );
}

#[test]
fn a_rule_for_one_net_beats_the_cell() {
    let net = rule("hv_a", NetScope::Net("HV_A".into()), 600_000);
    let got = verdict(&facing(2), vec![cell(300_000), net]);
    assert_eq!(got, Some((600_000, Some("user::hv_a".to_string()))));
}

#[test]
fn a_cell_outranks_a_class_rule_of_either_class() {
    let class = rule("hv", NetScope::Class("HV".into()), 700_000);
    assert_eq!(rule("x", NetScope::Class("HV".into()), 1).rank(), (2, 0));
    assert_eq!(cell(1).rank(), (2, 1));
    let got = verdict(&facing(2), vec![class, cell(300_000)]);
    assert_eq!(got, Some((300_000, Some("user::hv_lv".to_string()))));
}

#[test]
fn the_default_of_a_cell_is_what_the_pair_needs_without_it() {
    let b = facing(2);
    let rules = rules_for(&b, vec![cell(300_000)]);
    assert_eq!(rules.pair_default(1, 2, [None, None]), 500_000);
    let plain = rules_for(&b, vec![]);
    assert_eq!(plain.pair_default(2, 3, [None, None]), 200_000);
}

#[test]
fn a_cell_that_wins_is_not_flagged_shadowed_and_its_lowering_is_said() {
    let b = facing(2);
    let rules = rules_for(&b, vec![cell(300_000)]);
    let flags = rules.rule_flags(&b, &classes());
    assert!(
        !flags
            .iter()
            .any(|f| matches!(f.flag, RuleFlag::Shadowed { .. })),
        "{flags:?}"
    );
    assert!(
        flags.iter().any(|f| f.rule == "hv_lv"
            && matches!(&f.flag, RuleFlag::LowersClass { class, .. } if class == "HV")),
        "{flags:?}"
    );
}

#[test]
fn a_cell_naming_a_class_that_does_not_exist_is_said() {
    let b = facing(2);
    let typo = rule("x", NetScope::ClassPair("HV".into(), "Lv".into()), 300_000);
    let rules = rules_for(&b, vec![typo]);
    assert_eq!(
        rules.notes,
        ["DRC rule 'x': there is no net class 'Lv'; the rule checks nothing"]
    );
}

#[test]
fn why_this_value_names_the_cell_with_the_net_across() {
    let b = facing(2);
    let rules = rules_for(&b, vec![cell(300_000)]);
    let across =
        rules.effective_between(LimitKind::MinClearance, 1, Some(2), u32::MAX, [None, None]);
    assert_eq!(across.value_nm, Some(300_000));
    assert_eq!(across.source, Source::Rule("hv_lv".into()));
    let alone = rules.effective(LimitKind::MinClearance, 1, u32::MAX, [None, None]);
    assert_eq!(alone.value_nm, Some(500_000));
    let verdict = alone
        .chain
        .iter()
        .find(|c| c.name == "hv_lv")
        .map(|c| c.verdict);
    assert_eq!(verdict, Some(Verdict::OtherNets));
}
