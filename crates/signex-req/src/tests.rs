use std::collections::HashMap;

use serde_json::json;
use signex_bom::{BomRow, BomTable, PriceBook, PriceQuote, library_row_key};
use signex_types::check::{CheckReport, Finding, Severity};
use signex_types::net_class::NetClass;
use signex_types::pcb::{NetDef, PcbBoard, Point};
use signex_types::project::{RequirementEntry, SetupProfile};

use super::*;

const PROJECT: &str = "/p/demo.snxprj";
const BOARD: &str = "/p/demo.snxpcb";

fn entries(list: serde_json::Value) -> Vec<RequirementEntry> {
    serde_json::from_value(list).expect("a list")
}

fn board(width: f64, height: f64, nets: &[&str]) -> PcbBoard {
    let mut board = SetupProfile::Standard.pcb(2).new_board();
    board.outline = vec![
        Point::new(10.0, 10.0),
        Point::new(10.0 + width, 10.0),
        Point::new(10.0 + width, 10.0 + height),
        Point::new(10.0, 10.0 + height),
    ];
    board.nets = nets
        .iter()
        .enumerate()
        .map(|(i, name)| NetDef {
            number: i as u32 + 1,
            name: name.to_string(),
        })
        .collect();
    board
}

fn facts(list: serde_json::Value) -> Facts {
    Facts {
        project: PROJECT.into(),
        requirements: entries(list),
        unreadable_list: false,
        erc: None,
        drc: None,
        board: Board::NotListed,
        classes: Vec::new(),
        parts: None,
        row_states: HashMap::new(),
        bom: None,
        prices: PriceBook::default(),
        rails: Ok(std::collections::BTreeMap::new()),
        warnings: Vec::new(),
    }
}

fn read_board(board: PcbBoard) -> Board {
    Board::Read {
        path: BOARD.into(),
        board: Box::new(board),
    }
}

fn run_report(check: &str, severities: &[Severity]) -> CheckReport {
    let findings = severities
        .iter()
        .enumerate()
        .map(|(i, severity)| Finding {
            rule: "builtin::x".into(),
            severity: *severity,
            message: format!("finding {i}"),
            evidence: json!({}),
            items: Vec::new(),
            sheet: "/p/top.snxsch".into(),
            key: format!("k{i}"),
        })
        .collect();
    CheckReport::new(
        check,
        findings,
        Vec::new(),
        Vec::new(),
        vec!["a sheet was not read".into()],
    )
}

fn row(references: &[&str], fitted: u32, library_row: Option<&str>, mpn: &str) -> BomRow {
    BomRow {
        references: references.iter().map(|r| r.to_string()).collect(),
        qty: fitted,
        fitted_qty: fitted,
        library_row: library_row.map(str::to_string),
        custom: [("MPN".to_string(), mpn.to_string())].into_iter().collect(),
        ..BomRow::default()
    }
}

fn rules(report: &CheckReport) -> Vec<&str> {
    report.findings.iter().map(|f| f.rule.as_str()).collect()
}

#[test]
fn needs_follow_the_kinds_listed() {
    let list = entries(json!([
        {"id": "a", "kind": "erc_clean"},
        {"id": "b", "kind": "board_size", "max_width_mm": 1, "max_height_mm": 1},
        {"id": "c", "kind": "rail_budget"},
    ]));
    assert_eq!(
        Needs::of(&list),
        Needs {
            erc: true,
            board: true,
            ..Needs::default()
        }
    );
}

#[test]
fn a_met_requirement_is_listed_as_passed_and_raises_nothing() {
    let mut facts = facts(json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
    ]));
    facts.board = read_board(board(100.0, 80.0, &[]));
    let report = check_report(&facts);
    assert_eq!(report.check, "requirements");
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert_eq!(report.passed, ["req::size"]);
    assert_eq!(report.rule_set.as_ref().unwrap()["requirements"], 1);
}

#[test]
fn a_board_over_the_limit_fails_with_its_size_under_the_board() {
    let mut facts = facts(json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80,
         "severity": "warning"},
    ]));
    facts.board = read_board(board(100.5, 60.0, &[]));
    let report = check_report(&facts);
    let finding = &report.findings[0];
    assert_eq!(finding.rule, "req::size");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.sheet, BOARD);
    assert_eq!(
        finding.message,
        "The board is 100.5 x 60 mm, over the limit of 100 x 80 mm"
    );
    assert_eq!(finding.evidence["width_mm"], 100.5);
    assert_eq!(finding.evidence["kind"], "board_size");
    assert!(report.passed.is_empty());
}

#[test]
fn the_board_extent_is_the_size_board_size_measures() {
    assert_eq!(
        board_extent_mm(&read_board(board(48.5, 36.0, &[]))),
        Some((48.5, 36.0))
    );
    assert_eq!(board_extent_mm(&Board::NotListed), None);
    let mut open = board(10.0, 10.0, &[]);
    open.outline.truncate(2);
    assert_eq!(board_extent_mm(&read_board(open)), None, "no outline");
}

#[test]
fn a_board_requirement_with_no_board_is_a_finding_not_a_pass() {
    let report = check_report(&facts(json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
        {"id": "drc", "kind": "drc_clean"},
    ])));
    assert_eq!(rules(&report), ["req::drc", "req::size"]);
    assert!(report.passed.is_empty());
}

#[test]
fn erc_clean_counts_errors_and_warnings_unless_warnings_are_allowed() {
    let mut facts = facts(json!([
        {"id": "strict", "kind": "erc_clean"},
        {"id": "loose", "kind": "erc_clean", "allow_warnings": true},
    ]));
    facts.erc = Some(Ok(run_report("erc", &[Severity::Warning, Severity::Info])));
    let report = check_report(&facts);
    assert_eq!(rules(&report), ["req::strict"]);
    assert_eq!(report.findings[0].evidence["warnings"], 1);
    assert_eq!(report.findings[0].evidence["info"], 1);
    assert_eq!(report.passed, ["req::loose"]);
    assert_eq!(report.warnings, ["ERC: a sheet was not read"]);
}

#[test]
fn a_run_that_did_not_finish_fails_its_requirement() {
    let mut facts = facts(json!([{"id": "erc", "kind": "erc_clean"}]));
    facts.erc = Some(Err("the root sheet is missing".into()));
    let report = check_report(&facts);
    assert_eq!(
        report.findings[0].message,
        "The ERC run did not finish: the root sheet is missing"
    );
}

#[test]
fn drc_clean_is_filed_under_the_board() {
    let mut facts = facts(json!([{"id": "drc", "kind": "drc_clean"}]));
    facts.board = read_board(board(10.0, 10.0, &[]));
    facts.drc = Some(Ok(run_report("drc", &[Severity::Error])));
    let report = check_report(&facts);
    assert_eq!(report.findings[0].sheet, BOARD);
    assert_eq!(report.findings[0].evidence["errors"], 1);
}

#[test]
fn net_class_reports_each_net_outside_the_class_and_a_class_under_its_minimum() {
    let mut facts = facts(json!([
        {"id": "usb", "kind": "net_class", "nets": "USB_*", "class": "USB",
         "min_trace_width_mm": 0.2, "min_clearance_mm": 0.1},
    ]));
    facts.board = read_board(board(10.0, 10.0, &["USB_DP", "USB_DM", "GND"]));
    let mut usb = NetClass::named("USB");
    usb.nets = vec!["USB_DP".into()];
    usb.trace_width = 0.15;
    usb.clearance = 0.1;
    facts.classes = vec![usb];
    let report = check_report(&facts);
    let messages: Vec<&str> = report.findings.iter().map(|f| f.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "Class USB sets a track width of 0.15 mm, under 0.2 mm",
            "Net USB_DM is in no class, not USB",
        ]
    );
}

#[test]
fn net_class_with_no_matching_net_or_an_unknown_class_is_a_finding() {
    let mut facts = facts(json!([
        {"id": "can", "kind": "net_class", "nets": "CAN_*", "min_clearance_mm": 0.2},
        {"id": "pwr", "kind": "net_class", "nets": "*", "class": "Power"},
    ]));
    facts.board = read_board(board(10.0, 10.0, &["GND"]));
    let report = check_report(&facts);
    let messages: Vec<&str> = report.findings.iter().map(|f| f.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "No net of the board matches CAN_*",
            "The project has no net class Power",
        ]
    );
}

#[test]
fn parts_state_names_each_part_outside_the_allowed_states() {
    let mut facts = facts(json!([
        {"id": "parts", "kind": "parts_state", "states": ["released", "deprecated"]},
    ]));
    let released = library_row_key("lib", "r1");
    let draft = library_row_key("lib", "r2");
    facts.row_states = [
        (released.clone(), "released".to_string()),
        (draft.clone(), "draft".to_string()),
    ]
    .into_iter()
    .collect();
    facts.parts = Some(Ok(BomTable {
        rows: vec![
            row(&["R1"], 1, Some(&released), ""),
            row(&["R2"], 1, Some(&draft), ""),
            row(&["R3"], 1, None, ""),
            row(&["R4"], 1, Some("other/r9"), ""),
        ],
        metadata: Default::default(),
    }));
    let report = check_report(&facts);
    let messages: Vec<&str> = report.findings.iter().map(|f| f.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "R2 is draft, not released, deprecated",
            "R3 was not placed from a library row",
            "R4's library row is in no open library",
        ]
    );
    let keys: std::collections::HashSet<&str> =
        report.findings.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys.len(), 3, "each part has a key of its own");
}

/// Mockup row 38: a failed `parts_state` card counts parts, not BOM lines,
/// so a line grouping C1 and C2 counts twice on both sides of "of".
#[test]
fn a_parts_shortfall_counts_parts_not_bom_lines() {
    let mut parts = facts(json!([
        {"id": "parts", "kind": "parts_state", "states": ["released", "deprecated"]},
    ]));
    let released = library_row_key("lib", "r1");
    let draft = library_row_key("lib", "r2");
    parts.row_states = [
        (released.clone(), "released".to_string()),
        (draft.clone(), "draft".to_string()),
    ]
    .into_iter()
    .collect();
    parts.parts = Some(Ok(BomTable {
        rows: vec![
            row(&["R1"], 1, Some(&released), ""),
            row(&["C1", "C2"], 2, Some(&draft), ""),
            row(&["R3"], 1, None, ""),
            row(&["R4"], 1, Some("other/r9"), ""),
        ],
        metadata: Default::default(),
    }));
    let report = check_report(&parts);
    let found: Vec<&Finding> = report.findings.iter().collect();
    let grouped = found
        .iter()
        .find(|f| f.evidence["designator"] == json!("C1,C2"))
        .expect("the grouped line's finding");
    assert_eq!(grouped.evidence["count"], json!(2));
    assert_eq!(grouped.evidence["parts"], json!(5));
    let miss = |designator: &str, count, problem| PartMiss {
        designator: designator.into(),
        count,
        problem,
    };
    assert_eq!(
        parts_shortfall(&found),
        Some(PartsShortfall {
            missed: 4,
            of: 5,
            allowed: vec!["released".into(), "deprecated".into()],
            lines: vec![
                miss("C1,C2", 2, PartProblem::State("draft".into())),
                miss("R3", 1, PartProblem::Unplaced),
                miss("R4", 1, PartProblem::NoLibrary),
            ],
        })
    );
    assert_eq!(parts_shortfall(&[]), None);
}

/// Findings that are not per-part ones are no parts shortfall; the card
/// reads their messages instead.
#[test]
fn net_class_findings_are_no_parts_shortfall() {
    let mut nets = facts(json!([
        {"id": "usb", "kind": "net_class", "nets": "USB_*", "class": "USB",
         "min_trace_width_mm": 0.2},
    ]));
    nets.board = read_board(board(10.0, 10.0, &["USB_DP", "USB_DM", "GND"]));
    let mut usb = NetClass::named("USB");
    usb.nets = vec!["USB_DP".into()];
    usb.trace_width = 0.15;
    nets.classes = vec![usb];
    let report = check_report(&nets);
    let found: Vec<&Finding> = report.findings.iter().collect();
    assert_eq!(found.len(), 2, "two misses of one requirement");
    assert_eq!(parts_shortfall(&found), None);
}

#[test]
fn an_unknown_lifecycle_state_is_not_checked() {
    let mut facts = facts(json!([
        {"id": "parts", "kind": "parts_state", "states": ["approved"]},
    ]));
    facts.parts = Some(Ok(BomTable::default()));
    let report = check_report(&facts);
    assert!(report.findings[0].message.contains("approved"));
    assert!(report.passed.is_empty());
}

fn quote(currency: &str, breaks: &[(u32, f64)]) -> PriceQuote {
    PriceQuote {
        distributor: "D".into(),
        currency: currency.into(),
        stock: None,
        breaks: breaks.to_vec(),
        age: "1 d ago".into(),
    }
}

#[test]
fn bom_cost_prices_each_line_at_the_build_quantity() {
    let list = |max: f64| {
        json!([{"id": "cost", "kind": "bom_cost", "quantity": 100, "currency": "USD",
                "max_unit_cost": max}])
    };
    let make = |max: f64| {
        let mut facts = facts(list(max));
        facts.bom = Some(Ok(BomTable {
            rows: vec![row(&["R1", "R2"], 2, None, "RC0603")],
            metadata: Default::default(),
        }));
        facts.prices.by_mpn.insert(
            "RC0603".into(),
            vec![quote("USD", &[(1, 0.10), (100, 0.02)])],
        );
        facts
    };
    // 2 per board x 100 boards = 200 at 0.02 = 4.00, so 0.04 per board.
    assert_eq!(check_report(&make(0.04)).passed, ["req::cost"]);
    let over = check_report(&make(0.03));
    assert_eq!(over.findings[0].evidence["unit_cost"], 0.04);
    assert_eq!(
        over.findings[0].message,
        "The BOM costs 0.04 USD per board for 100 boards, over the budget of 0.03 USD"
    );
}

#[test]
fn a_line_without_a_price_in_the_currency_makes_the_cost_unknown() {
    let mut facts = facts(json!([
        {"id": "cost", "kind": "bom_cost", "quantity": 10, "currency": "USD", "max_unit_cost": 100},
    ]));
    facts.bom = Some(Ok(BomTable {
        rows: vec![
            row(&["R1"], 1, None, "A"),
            row(&["C1"], 1, None, "B"),
            row(&["U1"], 0, None, "C"),
        ],
        metadata: Default::default(),
    }));
    facts
        .prices
        .by_mpn
        .insert("B".into(), vec![quote("EUR", &[(1, 1.0)])]);
    let report = check_report(&facts);
    let finding = &report.findings[0];
    assert_eq!(
        finding.message,
        "The BOM cost is not known: 2 line(s) have no cached price in USD"
    );
    assert_eq!(finding.evidence["missing"], json!(["R1"]));
    assert_eq!(
        finding.evidence["other_currency"],
        json!([{"line": "C1", "currency": "EUR"}])
    );
}

#[test]
fn an_unreadable_or_repeated_entry_is_reported_as_not_checked() {
    let report = check_report(&facts(json!([
        {"id": "rails", "kind": "rail_budget"},
        {"kind": "erc_clean"},
        {"id": "size", "kind": "board_size", "max_width_mm": 1, "max_height_mm": 1},
        {"id": "size", "kind": "erc_clean"},
    ])));
    let rules = rules(&report);
    assert!(rules.contains(&"req::rails"), "{rules:?}");
    assert!(rules.contains(&"req::#2"), "{rules:?}");
    assert_eq!(rules.iter().filter(|r| **r == "req::size").count(), 2);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.message.contains("uses an id an earlier requirement has"))
    );
}

#[test]
fn a_repeated_id_never_leaves_its_rule_passed() {
    // The first `a` passes on its own; a readable or an unreadable repeat
    // takes it back out of `passed`, in either order.
    for list in [
        json!([
            {"id": "a", "kind": "erc_clean"},
            {"id": "a", "kind": "board_size", "max_width_mm": 1, "max_height_mm": 1},
        ]),
        json!([
            {"id": "a", "kind": "erc_clean"},
            {"id": "a", "kind": "rail_budget"},
        ]),
        json!([
            {"id": "a", "kind": "rail_budget"},
            {"id": "a", "kind": "erc_clean"},
        ]),
    ] {
        let mut facts = facts(list.clone());
        facts.erc = Some(Ok(run_report("erc", &[])));
        let report = check_report(&facts);
        assert!(report.passed.is_empty(), "{list}: {:?}", report.passed);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "req::a" && f.message.contains("uses an id an earlier")),
            "{list}"
        );
    }
}

#[test]
fn unreadable_entries_with_one_id_keep_apart_keys() {
    let report = check_report(&facts(json!([
        {"id": "x", "kind": "rail_budget"},
        {"id": "x", "kind": "rail_budget", "rail": "3V3"},
    ])));
    let keys = report.keys();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn a_list_that_cannot_be_read_and_an_empty_list_say_so() {
    let mut unreadable = facts(json!([]));
    unreadable.unreadable_list = true;
    assert_eq!(rules(&check_report(&unreadable)), ["req::requirements"]);
    let empty = check_report(&facts(json!([])));
    assert!(empty.findings.is_empty());
    assert_eq!(empty.warnings, ["The project lists no requirements"]);
}

#[test]
fn keys_hold_across_runs_whatever_was_measured() {
    let run = |width: f64| {
        let mut facts = facts(json!([
            {"id": "size", "kind": "board_size", "max_width_mm": 10, "max_height_mm": 10},
        ]));
        facts.board = read_board(board(width, 5.0, &[]));
        check_report(&facts).keys()
    };
    assert_eq!(run(12.0), run(14.0));
}

#[test]
fn a_turned_off_requirement_is_not_checked_needs_nothing_and_says_so() {
    let mut facts = facts(json!([
        {"id": "erc", "kind": "erc_clean", "enabled": false},
        {"id": "size", "kind": "board_size", "max_width_mm": 10, "max_height_mm": 10,
         "enabled": false},
        {"id": "size", "kind": "board_size", "max_width_mm": 10, "max_height_mm": 10},
    ]));
    assert_eq!(
        Needs::of(&facts.requirements),
        Needs {
            board: true,
            ..Needs::default()
        }
    );
    facts.board = read_board(board(5.0, 5.0, &[]));
    let report = check_report(&facts);
    // Its id is still taken: the second `size` is a finding, never a pass.
    assert_eq!(rules(&report), ["req::size"]);
    assert!(report.passed.is_empty(), "{:?}", report.passed);
    assert!(
        report
            .warnings
            .contains(&"Requirement erc is turned off, so it was not checked".to_string()),
        "{:?}",
        report.warnings
    );
}

#[test]
fn a_layer_count_passes_within_its_limits_and_says_the_count_when_not() {
    let list = json!([
        {"id": "two_to_four", "kind": "layer_count", "min_layers": 2, "max_layers": 4},
        {"id": "six_up", "kind": "layer_count", "min_layers": 6},
        {"id": "upside_down", "kind": "layer_count", "min_layers": 4, "max_layers": 2},
    ]);
    let mut facts = facts(list);
    assert_eq!(
        Needs::of(&facts.requirements),
        Needs {
            board: true,
            ..Needs::default()
        }
    );
    facts.board = read_board(SetupProfile::Custom.pcb(4).new_board());
    let report = check_report(&facts);
    assert_eq!(report.passed, ["req::two_to_four"]);
    let under = report
        .findings
        .iter()
        .find(|f| f.rule == "req::six_up")
        .expect("a finding");
    assert_eq!(under.evidence["count"], json!(4));
    assert_eq!(under.evidence["min_layers"], json!(6));
    assert_eq!(under.sheet, BOARD);
    assert!(rules(&report).contains(&"req::upside_down"));

    let mut no_board = facts.clone();
    no_board.board = Board::NotListed;
    assert!(
        check_report(&no_board).passed.is_empty(),
        "no board is no pass"
    );
}

#[test]
fn a_rail_budget_passes_on_one_project_and_fails_on_another() {
    let list = json!([
        {"id": "core", "kind": "rail_budget", "net": "+1V8", "voltage": 1.8,
         "margin_percent": 20},
    ]);
    let rail = |load: &str| {
        let mut facts = facts(list.clone());
        facts.rails = Ok(std::collections::BTreeMap::from([(
            "+1V8".to_string(),
            signex_types::project::RailSpec {
                voltage: 1.8,
                tolerance_percent: None,
            },
        )]));
        let mut source = row(&["U1"], 1, None, "LDO");
        source
            .custom
            .insert("Rail Supply +1V8".into(), "500mA".into());
        let mut sink = row(&["U2"], 1, None, "MCU");
        sink.custom.insert("Rail Load +1V8".into(), load.into());
        facts.parts = Some(Ok(BomTable {
            rows: vec![source, sink],
            ..BomTable::default()
        }));
        check_report(&facts)
    };
    let met = rail("0.3");
    assert_eq!(met.passed, ["req::core"]);
    assert!(met.findings.is_empty(), "{:?}", met.findings);
    let over = rail("450mA");
    assert!(over.passed.is_empty());
    assert_eq!(rules(&over), ["req::core"]);
    let evidence = &over.findings[0].evidence;
    assert_eq!(evidence["kind"], "rail_budget");
    assert_eq!(evidence["rail"]["net"], "+1V8");
    assert_eq!(evidence["rail"]["loads"][0]["designator"], "U2");
    assert_eq!(rail("450mA").keys(), rail("480mA").keys(), "the key holds");
}
