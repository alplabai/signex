//! #2199 (decisions 6817821301, 6818011610): DRC findings in the shared
//! check shape, their severities from the resolved rule model.

use std::collections::BTreeMap;

use serde_json::{Map, json};
use signex_types::check::{CheckReport, Severity};
use signex_types::pcb::{PadType, PcbBoard, PcbSetup, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_sketch::{BoardSketch, PcbKeepout};
use signex_types::project::DrcRuleSet;
use signex_types::sketch::attr::KeepoutKinds;

use super::findings::{DRC_CHECK, check_report};
use super::tests::{board, footprint, id, pad, track};
use super::*;

const BOARD: &str = "/p/board.snxpcb";
const RULE: i64 = 200_000;

fn names() -> BTreeMap<u32, String> {
    BTreeMap::from([(1, "GND".to_string()), (2, "VCC".to_string())])
}

/// Tracks of nets 1 and 2 on top copper, `pitch` mm apart centre to centre,
/// starting at `x` mm.
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

fn checked_with(b: &PcbBoard, rules: &DrcRules) -> CheckReport {
    check_report(&run(b, rules), &names(), BOARD, Vec::new(), Map::new())
}

fn checked(b: &PcbBoard) -> CheckReport {
    checked_with(b, &DrcRules::new(RULE))
}

#[test]
fn a_clearance_finding_carries_the_shared_fields_and_drc_evidence() {
    let report = checked(&pair(0.0, 0.4));
    assert_eq!(report.check, DRC_CHECK);
    assert_eq!(report.findings.len(), 1, "{report:?}");
    let f = &report.findings[0];
    assert_eq!(f.rule, "builtin::clearance");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.sheet, BOARD);
    assert_eq!(f.items, [id(2001).to_string(), id(2002).to_string()]);
    assert_eq!(f.key.len(), 32);
    assert_eq!(
        f.message, "clearance 0.15 mm, needs 0.2 mm, GND / VCC",
        "{f:?}"
    );
    assert_eq!(
        f.evidence,
        json!({
            "nets": ["GND", "VCC"],
            "layers": ["TopCopper"],
            "at": [0.0, 0.2],
            "measured_nm": 150_000,
            "required_nm": 200_000,
            "rule_source": "board_setup",
        })
    );
    assert!(report.waived.is_empty() && report.lapsed.is_empty());
}

#[test]
fn a_short_and_an_unconnected_link_are_findings_of_their_rules() {
    let short = checked(&pair(0.0, 0.1));
    assert_eq!(short.findings[0].rule, "builtin::short");
    assert_eq!(short.findings[0].message, "short, GND / VCC");
    assert_eq!(short.findings[0].evidence["measured_nm"], 0);

    let fp = footprint(
        1,
        vec![
            pad(1, (0.0, 0.0), 1, PadType::Smd, "TopCopper"),
            pad(2, (10.0, 0.0), 1, PadType::Smd, "TopCopper"),
        ],
    );
    let open = checked(&board(vec![fp], vec![], vec![]));
    let f = &open.findings[0];
    assert_eq!(f.rule, "builtin::unconnected");
    assert_eq!(f.severity, Severity::Warning);
    assert_eq!(f.message, "link of 10 mm not routed, GND / GND");
    assert_eq!(
        f.items,
        [
            format!("{}/{}", id(1001), id(1)),
            format!("{}/{}", id(1001), id(2))
        ]
    );
    assert_eq!(f.evidence["layers"], json!([]));
    assert!(f.evidence.get("rule_source").is_none(), "{}", f.evidence);
}

#[test]
fn a_keepout_finding_names_its_keepout() {
    let mut b = board(
        vec![],
        vec![track(1, (15.0, 25.0), (35.0, 25.0), 0, "TopCopper")],
        vec![],
    );
    let corners = [(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)];
    b.sketch = Some(BoardSketch {
        keepouts: vec![PcbKeepout {
            uuid: id(0x900),
            boundary: corners.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            layers: vec![PcbLayer::Copper(CopperLayer::Top)],
            forbids: KeepoutKinds {
                no_routing: true,
                ..KeepoutKinds::default()
            },
        }],
        ..BoardSketch::default()
    });
    let report = checked(&b);
    let f = &report.findings[0];
    assert_eq!(f.rule, "builtin::keepout");
    assert_eq!(f.evidence["keepout"], id(0x900).to_string());
    assert_eq!(f.evidence["nets"], json!(["net 0", "net 0"]));
}

/// A written rule's finding carries `user::<name>` and the rule's severity.
#[test]
fn a_finding_of_a_written_rule_carries_its_id_and_severity() {
    let rule = |name: &str, severity, limit| UserRule {
        name: name.into(),
        severity,
        message: None,
        nets: NetScope::Board,
        layers: None,
        pair: None,
        limits: vec![limit],
    };
    let b = pair(0.0, 1.0);
    let rules = DrcRules::new(RULE).with_user_rules(
        &b,
        &[],
        [
            rule("wide", Severity::Warning, Limit::MinClearance(900_000)),
            rule("thick", Severity::Info, Limit::MinTrackWidth(300_000)),
        ],
    );
    let report = checked_with(&b, &rules);
    let of = |id: &str| {
        report
            .findings
            .iter()
            .filter(|f| f.rule == id)
            .map(|f| (f.severity, f.evidence["rule_source"].clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(of("user::wide"), [(Severity::Warning, json!("written"))]);
    assert_eq!(of("user::thick").len(), 2, "{report:?}");
    assert!(of("user::thick").iter().all(|(s, _)| *s == Severity::Info));
}

/// A layer severity overrides the built-in default; `off` drops the
/// finding, as a rule switched off reports nothing.
#[test]
fn a_finding_takes_the_resolved_severity_and_off_reports_nothing() {
    let layer = |id: &str, sev: &str| DrcRuleSet {
        severity: BTreeMap::from([(id.to_string(), sev.to_string())]),
        ..DrcRuleSet::default()
    };
    let mut rules = DrcRules::new(RULE);
    rules.severities = resolve_severities(Some(&layer("builtin::clearance", "info")), None);
    let report = checked_with(&pair(0.0, 0.4), &rules);
    assert_eq!(report.findings[0].severity, Severity::Info);

    rules.severities = resolve_severities(None, Some(&layer("builtin::clearance", "off")));
    let report = checked_with(&pair(0.0, 0.4), &rules);
    assert!(report.findings.is_empty(), "{report:?}");
}

#[test]
fn a_net_class_clearance_is_named_as_the_rule_source() {
    let mut rules = DrcRules::new(RULE);
    rules.net_clearance.insert(2, 500_000);
    let report = checked_with(&pair(0.0, 0.6), &rules);
    assert_eq!(report.findings[0].evidence["rule_source"], "net_class");
    rules.net_clearance.clear();
    rules.from_default = true;
    let report = checked_with(&pair(0.0, 0.4), &rules);
    assert_eq!(report.findings[0].evidence["rule_source"], "default");
}

#[test]
fn a_finding_keeps_its_key_when_its_items_move() {
    let here = checked(&pair(0.0, 0.4));
    let moved = checked(&pair(40.0, 0.35));
    assert_ne!(here.findings[0].evidence, moved.findings[0].evidence);
    assert_eq!(here.keys(), moved.keys());
    // Another board file, another key.
    let elsewhere = check_report(
        &run(&pair(0.0, 0.4), &DrcRules::new(RULE)),
        &names(),
        "/p/other.snxpcb",
        Vec::new(),
        Map::new(),
    );
    assert_ne!(here.keys(), elsewhere.keys());
    // Another rule over the same items, another key.
    let short = checked(&pair(0.0, 0.1));
    assert_ne!(here.keys(), short.keys());
}

#[test]
fn a_keepout_is_part_of_the_key() {
    let fenced = |keepout: u128| {
        let mut b = board(
            vec![],
            vec![track(1, (15.0, 25.0), (35.0, 25.0), 0, "TopCopper")],
            vec![],
        );
        let corners = [(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)];
        b.sketch = Some(BoardSketch {
            keepouts: vec![PcbKeepout {
                uuid: id(keepout),
                boundary: corners.iter().map(|&(x, y)| Point::new(x, y)).collect(),
                layers: vec![PcbLayer::Copper(CopperLayer::Top)],
                forbids: KeepoutKinds {
                    no_routing: true,
                    ..KeepoutKinds::default()
                },
            }],
            ..BoardSketch::default()
        });
        checked(&b).keys()
    };
    assert_ne!(fenced(0x900), fenced(0x901));
}

#[test]
fn an_item_that_could_not_be_checked_is_a_warning() {
    let mut bad = track(1, (0.0, 0.0), (1.0, 0.0), 1, "TopCopper");
    bad.start = Point::new(f64::NAN, 0.0);
    let report = check_report(
        &run(&board(vec![], vec![bad], vec![]), &DrcRules::new(RULE)),
        &names(),
        BOARD,
        vec!["first".into()],
        Map::new(),
    );
    assert_eq!(report.warnings.len(), 2, "{:?}", report.warnings);
    assert_eq!(report.warnings[0], "first");
    assert!(
        report.warnings[1].starts_with(&format!("DRC: track {} could not be checked: ", id(2001))),
        "{:?}",
        report.warnings
    );
}

#[test]
fn what_the_rule_text_said_is_a_warning() {
    let mut rules = DrcRules::new(RULE);
    rules.notes = vec!["note one".into()];
    rules.remarks = vec!["remark one".into()];
    rules.texts_not_applied = 1;
    rules.rules_not_applied = 2;
    rules.severities = resolve_severities(
        Some(&DrcRuleSet {
            severity: BTreeMap::from([("builtin::nope".to_string(), "error".to_string())]),
            ..DrcRuleSet::default()
        }),
        None,
    );
    rules.from_default = true;
    let report = checked_with(&board(vec![], vec![], vec![]), &rules);
    assert_eq!(
        report.warnings,
        [
            "DRC rules: ignoring unrecognised entry (severity builtin::nope = error)",
            "note one",
            "remark one",
            "DRC rules: the rule text was not applied (syntax error)",
            "DRC rules: 2 written rule(s) not applied",
            "DRC: No clearance rule set on this board; checked with 0.2 mm. Set it in Board Setup.",
        ]
    );
}

#[test]
fn the_rule_set_says_the_clearance_the_run_used() {
    let mut b = pair(0.0, 1.0);
    b.setup = Some(PcbSetup {
        grid_size: 0.0,
        trace_width: 0.0,
        via_diameter: 0.0,
        via_drill: 0.0,
        clearance: 0.3,
        track_min_width: 0.0,
        via_min_diameter: 0.0,
        via_min_drill: 0.0,
    });
    let rules = DrcRules::for_board(&b, &[]).unwrap();
    let mut given = Map::new();
    given.insert("source".into(), json!("machine"));
    let report = check_report(&run(&b, &rules), &names(), BOARD, Vec::new(), given);
    assert_eq!(
        report.rule_set,
        Some(json!({"source": "machine", "clearance_nm": 300_000, "from_default": false}))
    );
}

#[test]
fn the_same_board_gives_the_same_report_errors_first() {
    let mut b = pair(0.0, 0.4);
    b.footprints.push(footprint(
        1,
        vec![
            pad(1, (50.0, 0.0), 1, PadType::Smd, "TopCopper"),
            pad(2, (60.0, 0.0), 1, PadType::Smd, "TopCopper"),
        ],
    ));
    let first = checked(&b);
    assert_eq!(first, checked(&b));
    let severities: Vec<Severity> = first.findings.iter().map(|f| f.severity).collect();
    assert_eq!(severities, [Severity::Error, Severity::Warning]);
}
