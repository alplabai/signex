//! Generate (mockup row 201): sections and options, what a block records,
//! the requirements table, the summary counts, `--fail-on` and the write
//! rule. The whole report is in `tests_generate_report`.

use serde_json::json;
use signex_types::check::{CheckReport, Finding, Severity, WaivedFinding};
use signex_types::project::RequirementEntry;

use crate::generate::{
    BomFacts, ChecksSource, Fact, FailOn, GenerateOptions, GenerateStatus, ProjectFacts,
    RequirementResult, Section, SheetFact, Template, WriteDecision, block_fields, cites,
    parse_fail_on, parse_sections, requirement_rows, statement, write_decision,
};
use crate::{Annotation, AnnotationKind, BlockId, scan};

/// The blocks of a small notebook: two measurements, a calculation, two
/// decisions and two issues, citing the requirements by id.
pub(crate) const NOTEBOOK: &str = r#"#import "@signex/notebook:0.1.0": *
= Power stage
#annotation(kind: "calculation", id: "CALC-2.1", title: "Divider for REQ-001")[R7 and R8 set #net("+3V3"), see #component("R7") and #component("R8").]
#annotation(kind: "measurement", id: "MEAS-007", value: "3.310", unit: "V", status: "done", title: "Output for REQ-001")[Measured on #component("U3").]
#annotation(kind: "measurement", id: "MEAS-010", value: "310", unit: "mV", status: "done", title: "Input ripple REQ-005")[Too high at #component("C14").]
#annotation(kind: "decision", id: "DEC-002", status: "accepted", title: "Buck at 500 kHz")[Chosen for REQ-003.]
#annotation(kind: "decision", id: "DEC-003", status: "proposed", title: "Bigger inductor")[Not taken yet.]
#annotation(kind: "issue", id: "ISS-004", status: "open", title: "Footprint check")[Check the pad size.]
#annotation(kind: "issue", id: "ISS-005", status: "open", title: "Input ripple")[Add a capacitor for REQ-005.]
#annotation(kind: "issue", id: "ISS-001", status: "resolved", title: "Old issue")[Done.]
"#;

pub(crate) fn blocks() -> Vec<Annotation> {
    scan(NOTEBOOK).0
}

pub(crate) fn requirement(id: &str, kind: serde_json::Value) -> RequirementEntry {
    let mut raw = kind;
    raw["id"] = json!(id);
    RequirementEntry::read(raw)
}

pub(crate) fn requirements() -> Vec<RequirementEntry> {
    vec![
        requirement("REQ-001", json!({"kind": "erc_clean"})),
        requirement(
            "REQ-003",
            json!({"kind": "drc_clean", "allow_warnings": true}),
        ),
        requirement(
            "REQ-005",
            json!({"kind": "board_size", "max_width_mm": 50.0, "max_height_mm": 40.0}),
        ),
    ]
}

pub(crate) fn finding(rule: &str, severity: Severity, message: &str) -> Finding {
    Finding {
        rule: rule.to_string(),
        severity,
        message: message.to_string(),
        evidence: json!({}),
        items: Vec::new(),
        sheet: "power.snxsch".to_string(),
        key: format!("{rule}:{message}"),
    }
}

/// REQ-001 and REQ-003 pass, REQ-005 fails.
pub(crate) fn requirement_report() -> CheckReport {
    let failed = finding("req::REQ-005", Severity::Error, "Board is 60 × 40 mm");
    CheckReport::new(
        "requirements",
        vec![failed],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .with_passed(vec!["req::REQ-001".into(), "req::REQ-003".into()])
}

/// ERC with two warnings and a waived finding; DRC clean.
pub(crate) fn erc_report() -> CheckReport {
    let waived = WaivedFinding {
        finding: finding("builtin::unused_pin", Severity::Warning, "U3.7 unused"),
        waiver: json!({"by": "test"}),
    };
    let findings = vec![
        finding("builtin::a", Severity::Warning, "first warning"),
        finding("builtin::b", Severity::Warning, "second warning"),
    ];
    CheckReport::new("erc", findings, vec![waived], Vec::new(), Vec::new())
}

pub(crate) fn facts() -> ProjectFacts {
    ProjectFacts {
        project_file: "buck-3v3.snxprj".into(),
        name: "buck-3v3".into(),
        revision: Some("B".into()),
        variant: "Base".into(),
        sheets: vec![
            SheetFact {
                name: "power".into(),
                file: "power.snxsch".into(),
                depth: 0,
                parts: 21,
            },
            SheetFact {
                name: "mcu".into(),
                file: "mcu.snxsch".into(),
                depth: 1,
                parts: 18,
            },
            SheetFact {
                name: "io".into(),
                file: "io.snxsch".into(),
                depth: 1,
                parts: 9,
            },
        ],
        power_nets: vec!["+3V3".into(), "VIN".into()],
        requirements: requirements(),
        blocks: Fact::read(blocks(), "design.snxnbk, 8 blocks"),
        requirement_check: Fact::read(requirement_report(), "check_requirements run 3"),
        erc: Fact::read(erc_report(), "check_erc run 1"),
        drc: Fact::read(
            CheckReport::new("drc", Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            "check_drc run 2",
        ),
        bom: Fact::read(
            BomFacts {
                lines: 21,
                parts: 48,
                cost: None,
                unpriced: vec!["R7".into()],
                no_stock: Vec::new(),
            },
            "BOM of variant Base",
        ),
        simulation: Fact::not_read("no simulation run is recorded"),
        outputs: Fact::not_read("no output run is recorded"),
        changes: Fact::not_read("the version history is not read"),
        stale_references: 0,
    }
}

#[test]
fn sections_have_the_drawn_tokens_in_report_order() {
    let tokens: Vec<&str> = Section::ALL.iter().map(|s| s.token()).collect();
    assert_eq!(
        tokens,
        [
            "summary",
            "requirements",
            "architecture",
            "decisions",
            "bom",
            "checks",
            "simulation",
            "outputs",
            "open-items",
            "changelog"
        ]
    );
    for section in Section::ALL {
        assert_eq!(Section::parse(section.token()), Some(section));
        assert!(!section.heading().is_empty());
    }
    assert_eq!(Section::parse("Summary"), None);
    assert_eq!(
        Section::Requirements.heading(),
        "Requirements and how each was met"
    );
}

#[test]
fn a_section_list_reads_in_report_order_once_each() {
    assert_eq!(
        parse_sections("checks, summary,checks"),
        Ok(vec![Section::Summary, Section::Checks])
    );
    assert_eq!(parse_sections("all"), Ok(Section::ALL.to_vec()));
    assert!(parse_sections(" , ").is_err());
    let refused = parse_sections("summary,appendix").unwrap_err();
    assert!(refused.contains("`appendix`"), "{refused}");
}

#[test]
fn the_default_options_are_the_drawn_ones() {
    let options = GenerateOptions::default();
    assert_eq!(options.sections, Section::ALL.to_vec());
    assert_eq!(options.template, Template::DesignReportA4);
    assert_eq!(options.checks, ChecksSource::RunNow);
    assert_eq!((options.document, options.revision), (None, None));
}

#[test]
fn toggling_a_section_keeps_report_order() {
    let options = GenerateOptions::default().toggled(Section::Bom);
    assert!(!options.has(Section::Bom));
    assert_eq!(options.sections.len(), 9);
    let options = options.toggled(Section::Bom);
    assert_eq!(options.sections, Section::ALL.to_vec());
}

#[test]
fn checks_source_and_template_tokens_round_trip() {
    for source in ChecksSource::ALL {
        assert_eq!(ChecksSource::parse(source.token()), Some(source));
    }
    assert_eq!(ChecksSource::parse("now"), None);
    assert_eq!(
        Template::parse("design-report-a4"),
        Some(Template::DesignReportA4)
    );
    assert_eq!(Template::parse("letter"), None);
}

#[test]
fn a_block_gives_its_typed_id_and_reading() {
    let blocks = blocks();
    let measurement = blocks
        .iter()
        .find(|b| b.kind == AnnotationKind::Measurement)
        .unwrap();
    let fields = block_fields(measurement);
    assert_eq!(fields.id, BlockId::parse("MEAS-007"));
    assert_eq!(fields.reading().as_deref(), Some("3.310 V"));
    let decision = blocks
        .iter()
        .find(|b| b.kind == AnnotationKind::Decision)
        .unwrap();
    assert_eq!(block_fields(decision).id, BlockId::parse("DEC-002"));
    assert_eq!(block_fields(decision).reading(), None);
}

#[test]
fn an_id_of_another_kind_is_not_the_blocks_id() {
    let (blocks, _) = scan(r#"#annotation(kind: "issue", id: "DEC-001")[Wrong prefix.]"#);
    assert_eq!(block_fields(&blocks[0]).id, None);
}

#[test]
fn a_block_cites_a_requirement_only_as_a_whole_word() {
    let (blocks, _) = scan(r#"#annotation(kind: "note", title: "About REQ-10")[Also REQ-2.]"#);
    assert!(cites(&blocks[0], "REQ-10"));
    assert!(cites(&blocks[0], "REQ-2"));
    assert!(!cites(&blocks[0], "REQ-1"));
    assert!(!cites(&blocks[0], ""));
}

#[test]
fn requirement_rows_take_their_result_from_the_check_report() {
    let report = requirement_report();
    let rows = requirement_rows(&requirements(), Some(&report), &blocks());
    let results: Vec<(&str, RequirementResult)> =
        rows.iter().map(|r| (r.id.as_str(), r.result)).collect();
    assert_eq!(
        results,
        [
            ("REQ-001", RequirementResult::Pass),
            ("REQ-003", RequirementResult::Pass),
            ("REQ-005", RequirementResult::Fail),
        ]
    );
    let unchecked = requirement_rows(&requirements(), None, &[]);
    assert!(
        unchecked
            .iter()
            .all(|r| r.result == RequirementResult::NotChecked)
    );
}

#[test]
fn measurements_are_the_evidence_and_parts_and_calculations_meet_it() {
    let rows = requirement_rows(&requirements(), None, &blocks());
    let first = &rows[0];
    assert_eq!(first.evidence, ["MEAS-007 3.310 V"]);
    assert_eq!(first.met_by, ["R7", "R8", "U3", "CALC-2.1"]);
    // REQ-003 has no measurement: its decision meets it, no evidence.
    assert_eq!(rows[1].met_by, ["DEC-002"]);
    assert!(rows[1].evidence.is_empty());
}

#[test]
fn a_calculation_is_the_evidence_when_nothing_was_measured() {
    let (blocks, _) = scan(
        r#"#annotation(kind: "calculation", id: "CALC-2.4", title: "Junction for REQ-003")[#component("U3") runs at 79.8 C.]"#,
    );
    let rows = requirement_rows(&requirements(), None, &blocks);
    assert_eq!(rows[1].evidence, ["CALC-2.4"]);
    assert_eq!(rows[1].met_by, ["U3"]);
}

#[test]
fn an_entry_with_no_id_is_named_by_its_position() {
    let entries = vec![RequirementEntry::read(json!({"kind": "erc_clean"}))];
    let rows = requirement_rows(&entries, None, &[]);
    assert_eq!(rows[0].id, "#1");
    let unreadable = vec![RequirementEntry::read(
        json!({"id": "REQ-9", "kind": "nope"}),
    )];
    let rows = requirement_rows(&unreadable, None, &[]);
    assert_eq!(rows[0].statement, "Could not be read from the project file");
}

#[test]
fn each_requirement_kind_has_a_statement() {
    let line = |kind: serde_json::Value| {
        let entry = requirement("R", kind);
        statement(&entry.requirement().expect("known").kind)
    };
    assert_eq!(
        line(json!({"kind": "erc_clean"})),
        "ERC has no errors and no warnings"
    );
    assert_eq!(
        line(json!({"kind": "drc_clean", "allow_warnings": true})),
        "DRC has no errors"
    );
    assert_eq!(
        line(json!({"kind": "board_size", "max_width_mm": 50.0, "max_height_mm": 40.5})),
        "Board at most 50 × 40.5 mm"
    );
    assert_eq!(
        line(json!({"kind": "parts_state", "states": ["released", "in_review"]})),
        "Every bought part is released or in_review"
    );
    assert_eq!(
        line(
            json!({"kind": "net_class", "nets": "USB*", "class": "usb", "min_trace_width_mm": 0.2})
        ),
        "Nets USB* in class usb, track at least 0.2 mm"
    );
    assert_eq!(
        line(
            json!({"kind": "bom_cost", "quantity": 100, "currency": "EUR", "max_unit_cost": 12.5})
        ),
        "BOM at most 12.5 EUR per board at 100 boards"
    );
}

#[test]
fn the_summary_counts_requirements_checks_and_open_issues() {
    let generated = crate::generate::generate(&facts(), &GenerateOptions::default());
    let summary = &generated.summary;
    assert_eq!(summary.command, "notebook.generate");
    assert_eq!(summary.output, "notebook/generated/design-report.snxnbk");
    assert_eq!(
        (summary.requirements.total, summary.requirements.pass),
        (3, 2)
    );
    assert_eq!(summary.requirements.failed, ["REQ-005"]);
    let erc = summary.checks.erc.as_ref().expect("erc read");
    assert_eq!((erc.errors, erc.warnings, erc.waived), (0, 2, 1));
    assert_eq!(summary.open_items, ["ISS-004", "ISS-005"]);
    assert!(!summary.written);
    assert_eq!(summary.status, GenerateStatus::Ok);
    assert_eq!(summary.exit_code(), 0);
}

#[test]
fn the_summary_names_each_source_and_what_was_not_read() {
    let generated = crate::generate::generate(&facts(), &GenerateOptions::default());
    let summary = &generated.summary;
    let not_read: Vec<&str> = summary.not_read.iter().map(|n| n.section).collect();
    assert_eq!(not_read, ["simulation", "outputs", "changelog"]);
    assert!(
        summary
            .sources
            .iter()
            .any(|s| s.section == "checks" && s.from == "check_erc run 1")
    );
    assert!(
        summary
            .sources
            .iter()
            .any(|s| s.section == "decisions" && s.from.starts_with("design.snxnbk"))
    );
}

#[test]
fn fail_on_requirements_makes_a_failing_requirement_exit_non_zero() {
    let mut summary = crate::generate::generate(&facts(), &GenerateOptions::default()).summary;
    assert!(!summary.apply_fail_on(&[FailOn::Checks]));
    assert_eq!(summary.exit_code(), 0);
    assert!(summary.apply_fail_on(&[FailOn::Requirements]));
    assert_eq!(summary.status, GenerateStatus::Fail);
    assert_eq!(summary.failed_on, ["requirements"]);
    assert_eq!(summary.exit_code(), 1);
}

#[test]
fn fail_on_checks_holds_on_an_erc_or_drc_error() {
    let mut facts = facts();
    let error = finding("builtin::short", Severity::Error, "VIN shorted to GND");
    facts.drc = Fact::read(
        CheckReport::new("drc", vec![error], Vec::new(), Vec::new(), Vec::new()),
        "check_drc run 4",
    );
    let mut summary = crate::generate::generate(&facts, &GenerateOptions::default()).summary;
    assert!(summary.apply_fail_on(&[FailOn::Checks]));
    assert_eq!(summary.failed_on, ["checks"]);
}

/// #3021: `--fail-on checks` fails when ERC or DRC was never run or read,
/// rather than reading the missing check as zero errors.
#[test]
fn fail_on_checks_fails_when_a_check_was_not_read() {
    let mut facts = facts();
    facts.erc = Fact::not_read("no ERC run is kept for the project");
    facts.drc = Fact::not_read("no DRC run is kept for the project");
    let mut summary = crate::generate::generate(&facts, &GenerateOptions::default()).summary;
    assert!(summary.checks.erc.is_none() && summary.checks.drc.is_none());
    assert!(summary.apply_fail_on(&[FailOn::Checks]));
    assert_eq!(summary.status, GenerateStatus::Fail);
    assert_eq!(summary.exit_code(), 1);
}

#[test]
fn a_fail_on_condition_reads_the_counts_of_an_answer() {
    assert!(FailOn::Requirements.holds_on(1, 0));
    assert!(!FailOn::Requirements.holds_on(0, 3));
    assert!(FailOn::Checks.holds_on(0, 3));
    assert!(!FailOn::Checks.holds_on(2, 0));
}

#[test]
fn a_fail_on_list_reads_its_two_conditions() {
    assert_eq!(
        parse_fail_on("requirements"),
        Ok(vec![FailOn::Requirements])
    );
    assert_eq!(
        parse_fail_on("checks,requirements,checks"),
        Ok(vec![FailOn::Checks, FailOn::Requirements])
    );
    assert!(parse_fail_on("").is_err());
    assert!(parse_fail_on("warnings").unwrap_err().contains("warnings"));
}

#[test]
fn an_existing_report_is_replaced_only_when_asked() {
    assert_eq!(write_decision(false, false), WriteDecision::Create);
    assert_eq!(write_decision(false, true), WriteDecision::Create);
    assert_eq!(write_decision(true, true), WriteDecision::Replace);
    assert_eq!(write_decision(true, false), WriteDecision::Refuse);
}

#[test]
fn a_fact_reads_back_its_value_source_or_reason() {
    let read: Fact<u8> = Fact::read(3, "here");
    assert_eq!(
        (read.value(), read.from(), read.why_not()),
        (Some(&3), Some("here"), None)
    );
    let missing: Fact<u8> = Fact::not_read("gone");
    assert_eq!(
        (missing.value(), missing.from(), missing.why_not()),
        (None, None, Some("gone"))
    );
    let mut facts = facts();
    facts.blocks = Fact::not_read("no notebook");
    assert!(facts.block_list().is_empty());
}
