//! The generated report as a file (mockup row 201): its header, that it
//! compiles, that two runs give the same file, the sections it holds and
//! the answer's JSON shape.

use serde_json::json;

use crate::generate::{
    Fact, GENERATED_NOTEBOOK, GenerateOptions, OutputFile, Section, SimulationRun, generate,
    generated_path,
};
use crate::tests_generate::facts;
use crate::{CompileOptions, NotebookFile, NotebookSources, compile, scan};

#[test]
fn the_report_is_its_own_file_under_notebook_generated() {
    assert_eq!(generated_path(), "notebook/generated/design-report.snxnbk");
    assert_eq!(GENERATED_NOTEBOOK, "generated/design-report");
    let report = generate(&facts(), &GenerateOptions::default());
    assert_eq!(report.path, generated_path());
}

#[test]
fn the_report_writes_the_lowest_format_token_and_reads_back() {
    let report = generate(&facts(), &GenerateOptions::default());
    assert!(
        report.text.starts_with("+++\nformat = \"snxnbk/1\"\n"),
        "{}",
        report.text
    );
    let file = NotebookFile::parse(&report.text).expect("reads back");
    assert_eq!(file.token(), "snxnbk/1");
    assert!(file.header.is_empty());
    // Unchanged, it saves back byte for byte.
    assert_eq!(file.write().as_deref(), Ok(report.text.as_str()));
}

#[test]
fn the_report_compiles_with_the_bundled_package_only() {
    let report = generate(&facts(), &GenerateOptions::default());
    assert!(
        report.summary.problems.is_empty(),
        "{:?}",
        report.summary.problems
    );
    assert!(report.summary.pages >= 1, "{} pages", report.summary.pages);
    let compiled = compile(
        &NotebookSources::new("generated/design-report.snxnbk", report.text.clone()),
        &CompileOptions::default(),
    );
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let pages = compiled.notebook.expect("pages").page_count();
    assert_eq!(pages, report.summary.pages);
}

#[test]
fn two_runs_on_the_same_project_give_the_same_notebook() {
    let options = GenerateOptions::default();
    let first = generate(&facts(), &options);
    let second = generate(&facts(), &options);
    assert_eq!(first.text, second.text);
    assert_eq!(first.summary, second.summary);
}

#[test]
fn every_section_is_written_under_its_heading_in_report_order() {
    let text = generate(&facts(), &GenerateOptions::default()).text;
    let mut at = 0;
    for section in Section::ALL {
        let heading = format!("\n= {}\n", section.heading());
        let found = text[at..]
            .find(&heading)
            .unwrap_or_else(|| panic!("{heading:?}"));
        at += found + heading.len();
    }
}

#[test]
fn only_the_picked_sections_are_written() {
    let options = GenerateOptions {
        sections: vec![Section::Requirements, Section::Checks],
        ..GenerateOptions::default()
    };
    let report = generate(&facts(), &options);
    assert!(report.text.contains("= Requirements and how each was met"));
    assert!(report.text.contains("= Check results"));
    assert!(!report.text.contains("= Summary"));
    assert_eq!(report.summary.sections, ["requirements", "checks"]);
    assert!(report.summary.not_read.is_empty());
}

#[test]
fn the_requirements_table_has_the_drawn_columns_and_results() {
    let text = generate(&facts(), &GenerateOptions::default()).text;
    assert!(text.contains(r#"strong("Req."), strong("Statement"), strong("Met by"), strong("Evidence"), strong("Result")"#));
    assert!(text.contains(r#""REQ-001", "ERC has no errors and no warnings", "R7, R8, U3, CALC-2.1", "MEAS-007 3.310 V", "Pass""#));
    assert!(text.contains(r#""REQ-005""#) && text.contains(r#""Fail""#));
    assert!(text.contains("2 of 3 requirements are met. Failing: REQ-005."));
}

#[test]
fn accepted_decisions_are_kept_as_written() {
    let text = generate(&facts(), &GenerateOptions::default()).text;
    let accepted = scan(crate::tests_generate::NOTEBOOK)
        .0
        .into_iter()
        .find(|b| b.title == "Buck at 500 kHz")
        .expect("DEC-002");
    assert!(text.contains(accepted.source.trim()));
    assert!(
        !text.contains("Bigger inductor"),
        "a proposed decision is not a key decision"
    );
}

#[test]
fn the_report_carries_the_document_number_and_revision_in_its_footer() {
    let options = GenerateOptions {
        document: Some("DN-0043".into()),
        revision: Some("A".into()),
        ..GenerateOptions::default()
    };
    let text = generate(&facts(), &options).text;
    assert!(text.contains(
        r#"#show: report.with(doc: "DN-0043", rev: "A", title: "buck-3v3 Design Report")"#
    ));
    let blank = generate(&facts(), &GenerateOptions::default()).text;
    assert!(blank.contains("report.with(doc: none, rev: none,"));
}

#[test]
fn a_source_that_was_not_read_says_why_in_its_section() {
    let text = generate(&facts(), &GenerateOptions::default()).text;
    assert!(text.contains("Simulation results could not be read: no simulation run is recorded."));
    assert!(text.contains("The version history could not be read"));
}

#[test]
fn recorded_simulation_runs_and_outputs_are_tabled() {
    let mut facts = facts();
    facts.simulation = Fact::read(
        vec![SimulationRun {
            name: "Load step".into(),
            kind: "transient".into(),
            measurements: vec![("Vout ripple".into(), "18.0 mV".into())],
        }],
        "2 saved runs",
    );
    facts.outputs = Fact::read(
        vec![OutputFile {
            path: "outputs/gerbers.zip".into(),
            sha256: "ab12".into(),
        }],
        "Export Job run",
    );
    let report = generate(&facts, &GenerateOptions::default());
    assert!(report.text.contains(r#""Vout ripple", "18.0 mV""#));
    assert!(report.text.contains(r#""outputs/gerbers.zip", "ab12""#));
    assert!(
        report.summary.problems.is_empty(),
        "{:?}",
        report.summary.problems
    );
}

#[test]
fn project_text_cannot_break_the_typst_source() {
    let mut facts = facts();
    facts.name = "evil\"] #panic(\"x\") [*".into();
    facts.sheets[0].name = "a]#b".into();
    let report = generate(&facts, &GenerateOptions::default());
    assert!(
        report.summary.problems.is_empty(),
        "{:?}",
        report.summary.problems
    );
    assert!(report.summary.pages >= 1);
}

#[test]
fn the_answer_has_the_drawn_json_fields() {
    let summary = generate(&facts(), &GenerateOptions::default()).summary;
    let value = serde_json::to_value(&summary).expect("serializes");
    for key in [
        "command",
        "project",
        "revision",
        "variant",
        "output",
        "pages",
        "sections",
        "requirements",
        "checks",
        "open_items",
        "stale_references",
        "written",
        "status",
    ] {
        assert!(value.get(key).is_some(), "missing {key}: {value}");
    }
    assert_eq!(value["status"], json!("ok"));
    assert_eq!(value["requirements"]["failed"], json!(["REQ-005"]));
    assert_eq!(
        value["checks"]["drc"],
        json!({"errors": 0, "warnings": 0, "waived": 0})
    );
    assert_eq!(value["sections"][8], json!("open-items"));
    assert!(value.get("failed_on").is_none() && value.get("problems").is_none());
}
