//! #2307 step 2b - written rules: precedence, scopes and the width and via
//! checks.

use signex_types::net_class::NetClass;
use signex_types::pcb::{NetDef, PcbBoard, PcbSetup, ViaType};
use signex_types::project::DrcRuleSet;
use signex_types::violation::Severity;

use super::tests::{board, id, track, via};
use super::*;

const RULE: i64 = 200_000;

/// Nets 1 `GND`, 2 `HV_A`, 3 `SIG`; `HV_*` nets are in class `HV`.
fn named(mut b: PcbBoard) -> PcbBoard {
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

fn clearance(name: &str, nets: NetScope, nm: i64) -> UserRule {
    rule(name, nets, vec![Limit::MinClearance(nm)])
}

/// Tracks of nets 1 and 2, `pitch` mm apart centre to centre (0.25 mm wide).
fn pair(pitch: f64) -> PcbBoard {
    named(board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (0.0, pitch), (10.0, pitch), 2, "TopCopper"),
        ],
        vec![],
    ))
}

fn rules_for(b: &PcbBoard, user: Vec<UserRule>) -> DrcRules {
    let mut rules = DrcRules::for_board(b, &classes()).unwrap();
    rules.clearance = RULE;
    rules.from_default = false;
    rules.severities = resolve_severities_with(None, None, &user);
    rules.with_user_rules(b, &classes(), user)
}

/// `(required, rule)` of the one violation, gap 0.15 mm (pitch 0.4 mm).
fn verdict(user: Vec<UserRule>) -> Option<(i64, Option<String>)> {
    verdict_of(&pair(0.4), user)
}

fn verdict_of(b: &PcbBoard, user: Vec<UserRule>) -> Option<(i64, Option<String>)> {
    let report = run(b, &rules_for(b, user));
    report
        .violations
        .first()
        .map(|v| (v.required_nm, v.rule.clone()))
}

/// Per-side precedence (#2307 review): a pair needs the stricter of its
/// two sides, so a rule for one net can loosen that net's own class value
/// but never the other net's, nor the setup value the other side falls
/// back to.
#[test]
fn a_pair_needs_the_stricter_side_so_a_net_rule_below_the_setup_does_not_loosen_it() {
    // GND to SIG (no class): 0.15 mm apart, setup 0.2 mm.
    let mut b = pair(0.4);
    b.segments[1].net = 3;
    let gnd = clearance("gnd", NetScope::Net("GND".into()), 100_000);
    assert_eq!(verdict_of(&b, vec![gnd]), Some((200_000, None)));
}

#[test]
fn on_equal_values_the_more_severe_side_wins_then_the_written_rule_is_named() {
    // GND's side falls back to the 0.2 mm setup (an error); HV_A's side is
    // `hv`, as severe: the written rule is named, whichever side it is.
    let hv = clearance("hv", NetScope::Class("HV".into()), 200_000);
    let want = Some((200_000, Some("user::hv".to_string())));
    assert_eq!(verdict(vec![hv.clone()]), want);
    let mut swapped = pair(0.4);
    swapped.segments[0].net = 2;
    swapped.segments[1].net = 1;
    assert_eq!(verdict_of(&swapped, vec![hv.clone()]), want);
    // A milder written rule never downgrades the other side's error.
    let mut mild = hv;
    mild.severity = Severity::Warning;
    assert_eq!(verdict(vec![mild.clone()]), Some((200_000, None)));
    assert_eq!(verdict_of(&swapped, vec![mild]), Some((200_000, None)));
}

#[test]
fn a_milder_rule_on_one_side_does_not_downgrade_the_other_sides_error() {
    let mut gnd = clearance("gnd", NetScope::Net("GND".into()), 500_000);
    gnd.severity = Severity::Info;
    let b = pair(0.4);
    let report = run(&b, &rules_for(&b, vec![gnd]));
    let v = &report.violations[0];
    assert_eq!(v.required_nm, 500_000);
    assert_eq!(report.severity_of(v), Some(Severity::Error));
}

#[test]
fn a_rule_whose_scope_matches_no_net_is_said() {
    let b = pair(0.4);
    let typo = clearance("hv", NetScope::Class("Hv".into()), 600_000);
    let rules = rules_for(&b, vec![typo]);
    assert_eq!(
        rules.notes,
        ["DRC rule 'hv': there is no net class 'Hv'; the rule checks nothing"]
    );
    let absent = vec![
        clearance("vbus", NetScope::Net("VBUS".into()), 600_000),
        clearance("usb", NetScope::Pattern("USB_*".into()), 600_000),
        clearance("all", NetScope::Board, 600_000),
    ];
    let rules = rules_for(&b, absent);
    assert!(rules.notes.is_empty(), "{:?}", rules.notes);
    assert_eq!(
        rules.remarks,
        [
            "DRC rule 'vbus': no net of this board is 'VBUS'; the rule checks nothing",
            "DRC rule 'usb': no net of this board matches 'USB_*'; the rule checks nothing",
        ]
    );
    assert!(rules_for(&b, vec![]).remarks.is_empty());
}

#[test]
fn an_inner_layer_rule_on_a_two_layer_board_lowers_nothing() {
    // The test board lists no stackup: it is read as two layers.
    let mut inner = clearance("inner", NetScope::Board, 150_000);
    inner.layers = Some(0x7FFF_FFFE);
    assert!(rules_for(&pair(0.4), vec![inner.clone()]).notes.is_empty());
    let mut four = pair(0.4);
    four.layers = ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| signex_types::pcb::LayerDef {
            id: i as u8,
            name: name.into(),
            layer_type: "signal".into(),
            display_name: String::new(),
        })
        .collect();
    assert_eq!(
        rules_for(&four, vec![inner]).notes,
        ["DRC rule 'inner' lowers net class 'HV' clearance 0.5 \u{2192} 0.15 mm"]
    );
}

#[test]
fn a_lowering_to_zero_is_written_as_zero() {
    let none = clearance("none", NetScope::Board, 0);
    assert_eq!(
        rules_for(&pair(0.4), vec![none]).notes,
        ["DRC rule 'none' lowers net class 'HV' clearance 0.5 \u{2192} 0 mm"]
    );
}

#[test]
fn a_lowering_note_compares_with_what_the_net_would_otherwise_need() {
    // HV's class asks 0.5 mm; with a 0.6 mm setup the net needs 0.6 mm, so
    // a 0.55 mm rule lowers it, from 0.6.
    let b = pair(0.4);
    let mut rules = DrcRules::for_board(&b, &classes()).unwrap();
    rules.clearance = 600_000;
    let rules = rules.with_user_rules(
        &b,
        &classes(),
        vec![clearance("r", NetScope::Board, 550_000)],
    );
    // The 0.6 mm is the setup's, not the class's: the note says so.
    assert_eq!(
        rules.notes,
        ["DRC rule 'r' lowers the board setup clearance for net class 'HV' 0.6 \u{2192} 0.55 mm"]
    );
}

fn four_layers(mut b: PcbBoard) -> PcbBoard {
    b.layers = ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| signex_types::pcb::LayerDef {
            id: i as u8,
            name: name.into(),
            layer_type: "signal".into(),
            display_name: String::new(),
        })
        .collect();
    b
}

const INNER: u32 = !(1 | 1 << 31);

fn via_findings(b: &PcbBoard, user: Vec<UserRule>) -> Vec<(ViolationKind, i64, Option<String>)> {
    run(b, &rules_for(b, user))
        .violations
        .iter()
        .map(|v| (v.kind, v.required_nm, v.rule.clone()))
        .collect()
}

#[test]
fn an_inner_layer_via_rule_does_not_lift_a_board_wide_minimum_off_the_outer_layers() {
    let mut inner = rule(
        "inner",
        NetScope::Board,
        vec![Limit::MinViaDiameter(200_000)],
    );
    inner.layers = Some(INNER);
    let all = rule(
        "vias",
        NetScope::Board,
        vec![Limit::MinViaDiameter(650_000)],
    );
    // A through via, 0.6 mm across, on a two-layer board and a four-layer one.
    let two = named(board(vec![], vec![], vec![via(1, (0.0, 0.0), 1)]));
    let want = [(
        ViolationKind::ViaDiameter,
        650_000,
        Some("user::vias".to_string()),
    )];
    assert_eq!(via_findings(&two, vec![all.clone(), inner.clone()]), want);
    let four = four_layers(two.clone());
    assert_eq!(via_findings(&four, vec![all.clone(), inner]), want);
    // On four layers a stricter inner rule holds on the inner layers.
    let mut strict = rule(
        "inner",
        NetScope::Board,
        vec![Limit::MinViaDiameter(700_000)],
    );
    strict.layers = Some(INNER);
    assert_eq!(
        via_findings(&four, vec![all.clone(), strict.clone()]),
        [(
            ViolationKind::ViaDiameter,
            700_000,
            Some("user::inner".to_string())
        )]
    );
    // On two layers the inner rule has no layer to hold on.
    assert_eq!(via_findings(&two, vec![all, strict]), want);
}

#[test]
fn an_inner_layer_clearance_rule_finds_nothing_on_a_two_layer_board() {
    let mut inner = clearance("inner", NetScope::Board, 500_000);
    inner.layers = Some(INNER);
    // Two through vias of GND and SIG, 0.4 mm apart: over the 0.2 mm setup.
    let two = named(board(
        vec![],
        vec![],
        vec![via(1, (0.0, 0.0), 1), via(2, (1.0, 0.0), 3)],
    ));
    assert!(run(&two, &rules_for(&two, vec![inner.clone()])).is_clean());
    // With inner layers the rule holds there.
    let four = four_layers(two);
    assert_eq!(
        via_findings(&four, vec![inner]),
        [(
            ViolationKind::Clearance,
            500_000,
            Some("user::inner".to_string())
        )]
    );
}

#[test]
fn a_written_rule_keeps_its_own_severity_when_severities_were_not_resolved_with_it() {
    let b = pair(0.4);
    let mut hv = clearance("hv", NetScope::Board, 300_000);
    hv.severity = Severity::Warning;
    // Built-in severities only, as a caller that never resolved the layers.
    let rules = DrcRules::new(RULE).with_user_rules(&b, &classes(), vec![hv]);
    let report = run(&b, &rules);
    let v = &report.violations[0];
    assert_eq!(v.rule.as_deref(), Some("user::hv"));
    assert_eq!(report.severity_of(v), Some(Severity::Warning));
}

#[test]
fn a_written_rule_below_a_class_value_is_said_once_per_rule_and_class() {
    let mut b = pair(0.4);
    b.nets.push(NetDef {
        number: 4,
        name: "HV_B".into(),
    });
    let base = clearance("base", NetScope::Board, 150_000);
    assert_eq!(
        rules_for(&b, vec![base.clone()]).notes,
        ["DRC rule 'base' lowers net class 'HV' clearance 0.5 \u{2192} 0.15 mm"]
    );
    // A class rule outranks `base` for HV nets: nothing is lowered.
    let hv = clearance("hv", NetScope::Class("HV".into()), 600_000);
    assert!(rules_for(&b, vec![base, hv]).notes.is_empty());
    assert!(rules_for(&b, vec![]).notes.is_empty());
}

#[test]
fn with_no_written_rule_the_setup_and_class_values_apply_as_before() {
    // HV_A's class asks 0.5 mm, over the 0.2 mm setup.
    assert_eq!(verdict(vec![]), Some((500_000, None)));
}

#[test]
fn a_rule_for_one_net_beats_its_class_even_when_looser() {
    let net = clearance("hv_a", NetScope::Net("HV_A".into()), 250_000);
    let class = clearance("hv", NetScope::Class("HV".into()), 600_000);
    // HV_A's side asks 0.25 mm, though its class rule asks 0.6 mm; GND's
    // side keeps the 0.2 mm setup value.
    let want = Some((250_000, Some("user::hv_a".to_string())));
    assert_eq!(verdict(vec![class.clone(), net.clone()]), want);
    assert_eq!(verdict(vec![net, class]), want);
}

#[test]
fn a_looser_rule_for_one_net_does_not_relax_the_other_nets_stricter_rule() {
    let gnd = clearance("gnd", NetScope::Net("GND".into()), 100_000);
    let hv = clearance("hv", NetScope::Class("HV".into()), 600_000);
    let want = Some((600_000, Some("user::hv".to_string())));
    assert_eq!(verdict(vec![gnd.clone(), hv.clone()]), want);
    assert_eq!(verdict(vec![hv, gnd.clone()]), want);
    // With no written HV rule, HV_A's class still asks its implicit 0.5 mm.
    assert_eq!(verdict(vec![gnd]), Some((500_000, None)));
}

/// `(required, rule)` of a min-width violation of one 0.25 mm `HV_A` track
/// on top.
fn width_verdict(user: Vec<UserRule>) -> Option<(i64, Option<String>)> {
    let mut b = widths(0.25);
    b.segments[0].net = 2;
    let report = run(&b, &rules_for(&b, user));
    report
        .violations
        .first()
        .map(|v| (v.required_nm, v.rule.clone()))
}

fn min_width(name: &str, nets: NetScope, nm: i64) -> UserRule {
    rule(name, nets, vec![Limit::MinTrackWidth(nm)])
}

#[test]
fn net_beats_class_beats_pattern_beats_layer_beats_board() {
    let board_wide = min_width("all", NetScope::Board, 700_000);
    let mut layer = min_width("top", NetScope::Board, 650_000);
    layer.layers = Some(1);
    let pattern = min_width("pat", NetScope::Pattern("HV_*".into()), 600_000);
    let class = min_width("cls", NetScope::Class("HV".into()), 550_000);
    let net = min_width("net", NetScope::Net("HV_A".into()), 500_000);
    let id = |n: &str| Some(format!("user::{n}"));
    let mut written = vec![board_wide, layer, pattern, class, net];
    for (nm, name) in [
        (500_000, "net"),
        (550_000, "cls"),
        (600_000, "pat"),
        (650_000, "top"),
        (700_000, "all"),
    ] {
        assert_eq!(width_verdict(written.clone()), Some((nm, id(name))));
        written.pop();
    }
}

#[test]
fn a_layer_condition_narrows_a_class_rule_and_wins_on_its_layer() {
    let everywhere = clearance("hv", NetScope::Class("HV".into()), 600_000);
    let mut top = clearance("hv_top", NetScope::Class("HV".into()), 250_000);
    top.layers = Some(1);
    let want = Some((250_000, Some("user::hv_top".to_string())));
    assert_eq!(verdict(vec![everywhere.clone(), top.clone()]), want);
    assert_eq!(verdict(vec![top.clone(), everywhere.clone()]), want);
    // On another layer only the class-wide rule covers HV_A.
    top.layers = Some(1 << 31);
    let want = Some((600_000, Some("user::hv".to_string())));
    assert_eq!(verdict(vec![everywhere, top]), want);
}

#[test]
fn a_written_rule_widens_the_search_only_for_the_nets_it_covers() {
    let b = pair(0.4);
    let gnd = clearance("gnd", NetScope::Net("GND".into()), 2_000_000);
    let rules = rules_for(&b, vec![gnd]);
    assert_eq!(rules.reach_of(1), 2_000_000);
    // HV_A: its class's 0.5 mm; SIG: the 0.2 mm setup.
    assert_eq!(rules.reach_of(2), 500_000);
    assert_eq!(rules.reach_of(3), RULE);
    let board_wide = rules_for(&b, vec![clearance("all", NetScope::Board, 900_000)]);
    assert_eq!(board_wide.reach_of(3), 900_000);
}

#[test]
fn equally_specific_rules_tie_to_the_strictest() {
    let loose = clearance("loose", NetScope::Board, 300_000);
    let strict = clearance("strict", NetScope::Board, 400_000);
    let want = Some((400_000, Some("user::strict".to_string())));
    assert_eq!(verdict(vec![loose.clone(), strict.clone()]), want);
    assert_eq!(verdict(vec![strict, loose]), want);
}

#[test]
fn a_board_wide_rule_ranks_above_the_implicit_class_value() {
    // The class asks 0.5 mm; a written board-wide 0.1 mm applies instead.
    assert_eq!(
        verdict(vec![clearance("all", NetScope::Board, 100_000)]),
        None
    );
}

#[test]
fn a_rule_for_a_net_not_on_the_board_changes_nothing() {
    let other = clearance("x", NetScope::Net("VBUS".into()), 100_000);
    assert_eq!(verdict(vec![other]), Some((500_000, None)));
}

#[test]
fn a_layer_rule_applies_only_on_its_layers() {
    let mut inner = clearance("inner", NetScope::Board, 100_000);
    inner.layers = Some(0x7FFF_FFFE);
    // Both tracks are on top: the inner rule does not apply.
    assert_eq!(verdict(vec![inner.clone()]), Some((500_000, None)));
    let mut outer = inner;
    outer.layers = Some(1 | 1 << 31);
    assert_eq!(verdict(vec![outer]), None);
}

#[test]
fn an_object_pair_rule_applies_only_between_those_objects() {
    let mut track_via = clearance("tv", NetScope::Board, 100_000);
    track_via.pair = Some([CopperObject::Via, CopperObject::Track]);
    assert_eq!(verdict(vec![track_via.clone()]), Some((500_000, None)));
    let mut track_track = track_via;
    track_track.pair = Some([CopperObject::Track, CopperObject::Track]);
    assert_eq!(verdict(vec![track_track]), None);
}

#[test]
fn a_written_rule_wider_than_the_implicit_ones_still_finds_its_pairs() {
    // 2 mm pitch: a 1.75 mm gap, far past the 0.5 mm class reach.
    let b = pair(2.0);
    let wide = clearance("wide", NetScope::Net("GND".into()), 2 * 1_000_000);
    let report = run(&b, &rules_for(&b, vec![wide]));
    assert_eq!(report.count(ViolationKind::Clearance), 1, "{report:?}");
}

fn widths(width_mm: f64) -> PcbBoard {
    let mut t = track(1, (0.0, 0.0), (10.0, 0.0), 3, "TopCopper");
    t.width = width_mm;
    named(board(vec![], vec![t], vec![]))
}

#[test]
fn a_track_width_rule_reports_a_track_outside_its_limits() {
    let limits = vec![Limit::MinTrackWidth(200_000), Limit::MaxTrackWidth(400_000)];
    let user = vec![rule("w", NetScope::Net("SIG".into()), limits)];
    for (mm, want) in [(0.2, 0), (0.4, 0), (0.199_999, 1), (0.400_001, 1)] {
        let b = widths(mm);
        let report = run(&b, &rules_for(&b, user.clone()));
        assert_eq!(report.count(ViolationKind::TrackWidth), want, "{mm}");
    }
    let b = widths(0.1);
    let v = run(&b, &rules_for(&b, user)).violations[0].clone();
    assert_eq!((v.measured_nm, v.required_nm), (100_000, 200_000));
    assert_eq!(v.items, [pcb_geom::ItemRef::Track(id(2001))]);
    assert_eq!(v.rule.as_deref(), Some("user::w"));
    assert_eq!(v.at, pcb_geom::PointNm::new(5_000_000, 0));
}

#[test]
fn the_narrowest_maximum_is_the_strictest() {
    let a = rule("a", NetScope::Board, vec![Limit::MaxTrackWidth(300_000)]);
    let b_rule = rule("b", NetScope::Board, vec![Limit::MaxTrackWidth(200_000)]);
    let b = widths(0.25);
    let report = run(&b, &rules_for(&b, vec![a, b_rule]));
    assert_eq!(report.violations[0].rule.as_deref(), Some("user::b"));
}

#[test]
fn via_rules_check_diameter_and_drill() {
    let user = vec![rule(
        "v",
        NetScope::Board,
        vec![Limit::MinViaDiameter(650_000), Limit::MinViaDrill(350_000)],
    )];
    let b = named(board(vec![], vec![], vec![via(1, (0.0, 0.0), 1)]));
    let report = run(&b, &rules_for(&b, user));
    let kinds: Vec<_> = report.violations.iter().map(|v| v.kind).collect();
    assert_eq!(kinds, [ViolationKind::ViaDiameter, ViolationKind::ViaDrill]);
    assert_eq!(report.violations[0].measured_nm, 600_000);
    assert_eq!(report.violations[1].measured_nm, 300_000);

    // A rule for inner layers only does not reach a blind via on top.
    let mut inner = rule("i", NetScope::Board, vec![Limit::MinViaDrill(350_000)]);
    inner.layers = Some(1 << 5);
    let mut blind = via(1, (0.0, 0.0), 1);
    blind.via_type = ViaType::Blind;
    blind.layers = vec!["TopCopper".into(), "Inner2Copper".into()];
    let b = named(board(vec![], vec![], vec![blind]));
    assert!(run(&b, &rules_for(&b, vec![inner])).is_clean());
}

#[test]
fn setup_minimums_are_not_checked_without_a_written_rule() {
    let mut b = widths(0.05);
    b.vias.push(via(1, (20.0, 0.0), 1));
    b.setup = Some(PcbSetup {
        grid_size: 0.0,
        trace_width: 0.0,
        via_diameter: 0.0,
        via_drill: 0.0,
        clearance: 0.2,
        track_min_width: 0.2,
        via_min_diameter: 1.0,
        via_min_drill: 0.5,
    });
    let rules = DrcRules::for_board(&b, &[]).unwrap();
    assert!(run(&b, &rules).is_clean());
}

#[test]
fn a_written_rule_takes_its_own_severity_and_a_layer_entry_overrides_it() {
    let mut user = clearance("hv", NetScope::Board, 300_000);
    user.severity = Severity::Warning;
    let b = pair(0.4);
    let mut rules = rules_for(&b, vec![user.clone()]);
    rules.severities = resolve_severities_with(None, None, std::slice::from_ref(&user));
    let report = run(&b, &rules);
    let v = &report.violations[0];
    assert_eq!(report.severity_of(v), Some(Severity::Warning));

    let mut project = DrcRuleSet::default();
    project.severity.insert("user::hv".into(), "info".into());
    project
        .severity
        .insert("builtin::clearance".into(), "off".into());
    rules.severities = resolve_severities_with(None, Some(&project), &[user.clone()]);
    let report = run(&b, &rules);
    assert_eq!(
        report.severity_of(&report.violations[0]),
        Some(Severity::Info)
    );
    assert!(rules.severities.unrecognised.is_empty());

    project.severity.insert("user::hv".into(), "off".into());
    rules.severities = resolve_severities_with(None, Some(&project), &[user]);
    assert_eq!(rules.severities.of_rule("user::hv"), None);
}

/// Naming every variant makes a new kind a compile error here; it must
/// then be given its place here, in `every` and in `ViolationKind::ALL`.
fn position(kind: ViolationKind) -> usize {
    match kind {
        ViolationKind::Short => 0,
        ViolationKind::Clearance => 1,
        ViolationKind::Unconnected => 2,
        ViolationKind::Keepout => 3,
        ViolationKind::TrackWidth => 4,
        ViolationKind::ViaDiameter => 5,
        ViolationKind::ViaDrill => 6,
    }
}

#[test]
fn all_lists_every_kind_in_declaration_order() {
    let every = [
        ViolationKind::Short,
        ViolationKind::Clearance,
        ViolationKind::Unconnected,
        ViolationKind::Keepout,
        ViolationKind::TrackWidth,
        ViolationKind::ViaDiameter,
        ViolationKind::ViaDrill,
    ];
    assert_eq!(ViolationKind::ALL.len(), every.len());
    for (i, kind) in every.into_iter().enumerate() {
        assert_eq!(position(kind), i);
        assert_eq!(ViolationKind::ALL[i], kind);
        assert_eq!(kind as usize, i, "declaration order");
    }
}
