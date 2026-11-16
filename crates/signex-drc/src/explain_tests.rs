//! #2398 - "Why this value?", rule flags, per-rule results and the scope
//! preview, by the resolution the check itself uses.

use signex_types::net_class::NetClass;
use signex_types::pcb::{NetDef, PcbBoard};
use signex_types::violation::Severity;

use super::tests::{board, track, via};
use super::*;

const SETUP: i64 = 200_000;

/// Nets 1 `GND`, 2 `HV_A`, 3 `SIG`; `HV_*` is class `HV` at 0.5 mm. Two
/// tracks 0.4 mm apart (GND, HV_A) and a via on SIG.
fn sample() -> PcbBoard {
    let mut b = board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (0.0, 0.4), (10.0, 0.4), 2, "TopCopper"),
        ],
        vec![via(1, (20.0, 20.0), 3)],
    );
    b.nets = [(1, "GND"), (2, "HV_A"), (3, "SIG")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    b
}

fn classes() -> Vec<NetClass> {
    let mut hv = NetClass::named("HV");
    hv.patterns = vec!["HV_*".into()];
    hv.clearance = 0.5;
    vec![hv]
}

fn rule(name: &str, nets: NetScope, limits: Vec<Limit>) -> UserRule {
    UserRule {
        name: name.into(),
        severity: Severity::Error,
        message: None,
        nets,
        layers: None,
        pair: None,
        limits,
    }
}

fn rules(b: &PcbBoard, user: Vec<UserRule>) -> DrcRules {
    let mut rules = DrcRules::for_board(b, &classes()).unwrap();
    rules.clearance = SETUP;
    rules.from_default = false;
    rules.with_user_rules(b, &classes(), user)
}

#[test]
fn the_effective_value_names_its_source_and_every_candidate() {
    let b = sample();
    let board_wide = rule("base", NetScope::Board, vec![Limit::MinClearance(300_000)]);
    let hv = rule(
        "hv",
        NetScope::Class("HV".into()),
        vec![Limit::MinClearance(800_000)],
    );
    let gnd = rule(
        "gnd",
        NetScope::Net("GND".into()),
        vec![Limit::MinClearance(100_000)],
    );
    let r = rules(&b, vec![board_wide, hv, gnd]);
    let top = 1;

    let hv_side = r.effective(LimitKind::MinClearance, 2, top, [None, None]);
    assert_eq!(hv_side.value_nm, Some(800_000));
    assert_eq!(hv_side.source, Source::Rule("hv".into()));
    let verdicts: Vec<(&str, Verdict)> = hv_side
        .chain
        .iter()
        .map(|c| (c.name.as_str(), c.verdict))
        .collect();
    assert_eq!(
        verdicts,
        [
            ("base", Verdict::LessSpecific),
            ("hv", Verdict::Won),
            ("gnd", Verdict::OtherNets),
        ]
    );
    assert_eq!(hv_side.implicit_nm, Some(500_000));

    // GND's own rule wins its side, though looser.
    let gnd_side = r.effective(LimitKind::MinClearance, 1, top, [None, None]);
    assert_eq!(gnd_side.value_nm, Some(100_000));
    assert_eq!(gnd_side.source, Source::Rule("gnd".into()));
}

#[test]
fn without_written_rules_the_class_or_the_setup_governs() {
    let b = sample();
    let r = rules(&b, vec![]);
    let hv = r.effective(LimitKind::MinClearance, 2, 1, [None, None]);
    assert_eq!((hv.value_nm, hv.source), (Some(500_000), Source::NetClass));
    let gnd = r.effective(LimitKind::MinClearance, 1, 1, [None, None]);
    assert_eq!(
        (gnd.value_nm, gnd.source),
        (Some(SETUP), Source::BoardSetup)
    );
    let width = r.effective(LimitKind::MinTrackWidth, 1, 1, [None, None]);
    assert_eq!((width.value_nm, width.source), (None, Source::Unlimited));
}

#[test]
fn a_violation_is_explained_side_by_side() {
    let b = sample();
    let r = rules(&b, vec![]);
    let report = run(&b, &r);
    let v = report
        .violations
        .iter()
        .find(|v| v.kind == ViolationKind::Clearance)
        .expect("0.15 mm gap against HV's 0.5 mm");
    let why = r.explain(v).unwrap();
    assert_eq!(why.kind, LimitKind::MinClearance);
    assert_eq!(why.required_nm, 500_000);
    let values: Vec<Option<i64>> = why.sides.iter().map(|s| s.value_nm).collect();
    assert_eq!(values, [Some(SETUP), Some(500_000)]);
}

#[test]
fn a_rule_that_never_wins_is_shadowed_and_equal_rules_that_disagree_conflict() {
    let b = sample();
    let wide = rule(
        "wide",
        NetScope::Pattern("HV_*".into()),
        vec![Limit::MinClearance(300_000)],
    );
    let hv = rule(
        "hv",
        NetScope::Class("HV".into()),
        vec![Limit::MinClearance(800_000)],
    );
    let a = rule(
        "a",
        NetScope::Net("SIG".into()),
        vec![Limit::MinClearance(300_000)],
    );
    let b_rule = rule(
        "b",
        NetScope::Net("SIG".into()),
        vec![Limit::MinClearance(400_000)],
    );
    let nowhere = rule(
        "vbus",
        NetScope::Net("VBUS".into()),
        vec![Limit::MinClearance(1)],
    );
    let r = rules(&b, vec![wide, hv, a, b_rule, nowhere]);
    let flags = r.rule_flags(&b, &classes());
    let has = |rule: &str, flag: RuleFlag| {
        flags.contains(&FlaggedRule {
            rule: rule.into(),
            flag,
        })
    };
    assert!(
        has("wide", RuleFlag::Shadowed { by: "hv".into() }),
        "{flags:?}"
    );
    assert!(has("a", RuleFlag::Shadowed { by: "b".into() }), "{flags:?}");
    assert!(
        has("a", RuleFlag::Conflict { with: "b".into() }),
        "{flags:?}"
    );
    assert!(
        has("b", RuleFlag::Conflict { with: "a".into() }),
        "{flags:?}"
    );
    assert!(has("vbus", RuleFlag::NoNets), "{flags:?}");
    assert!(!flags.iter().any(|f| f.rule == "hv"), "{flags:?}");
}

#[test]
fn the_flags_check_one_net_per_set_of_covering_rules() {
    let b = sample();
    let base = rule("base", NetScope::Board, vec![Limit::MinClearance(300_000)]);
    let gnd = rule(
        "gnd",
        NetScope::Net("GND".into()),
        vec![Limit::MinClearance(100_000)],
    );
    // HV_A and SIG are covered by `base` alone: HV_A answers for both.
    let r = rules(&b, vec![base.clone(), gnd]);
    assert_eq!(r.net_representatives(&b), [1u32, 2]);
    let r = rules(&b, vec![base]);
    assert_eq!(r.net_representatives(&b), [1u32]);
}

#[test]
fn a_rule_below_a_class_value_is_flagged_as_lowering_it() {
    let b = sample();
    let base = rule("base", NetScope::Board, vec![Limit::MinClearance(150_000)]);
    let r = rules(&b, vec![base]);
    let flags = r.rule_flags(&b, &classes());
    assert!(flags.contains(&FlaggedRule {
        rule: "base".into(),
        flag: RuleFlag::LowersClass {
            class: "HV".into(),
            from_nm: 500_000,
            to_nm: 150_000,
        },
    }));
}

#[test]
fn results_count_each_rules_findings_and_keep_the_worst() {
    let b = sample();
    let width = rule(
        "thick",
        NetScope::Board,
        vec![Limit::MinTrackWidth(300_000)],
    );
    let r = rules(&b, vec![width]);
    let report = run(&b, &r);
    let results = report.rule_results();
    let thick = results["user::thick"];
    assert_eq!(thick.violations, 2);
    assert_eq!(thick.worst, Some((250_000, 300_000)));
    assert_eq!(thick.margin_nm(), Some(-50_000));
}

#[test]
fn the_scope_preview_lists_the_objects_a_rule_applies_to() {
    let b = sample();
    let hv = rule(
        "hv",
        NetScope::Class("HV".into()),
        vec![Limit::MinClearance(800_000)],
    );
    let vias = rule("vias", NetScope::Board, vec![Limit::MinViaDrill(200_000)]);
    let r = rules(&b, vec![hv, vias]);
    let hv_items = r.scope_items(&b, "hv").unwrap();
    assert_eq!(hv_items, [pcb_geom::ItemRef::Track(b.segments[1].uuid)]);
    let via_items = r.scope_items(&b, "vias").unwrap();
    assert_eq!(via_items, [pcb_geom::ItemRef::Via(b.vias[0].uuid)]);
    assert!(r.scope_items(&b, "nope").is_none());
}
