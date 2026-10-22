use super::*;
use crate::{ExportContext, ProjectMetadata};

fn ctx() -> ExportContext {
    ExportContext {
        sheets: Vec::new(),
        metadata: ProjectMetadata::default(),
        netlist: None,
    }
}

fn row(sheet: &str, sev: ErcReportSeverity, rule: &str, msg: &str, x: f64, y: f64) -> ErcReportRow {
    ErcReportRow {
        sheet_key: sheet.into(),
        sheet_name: sheet.into(),
        severity: sev,
        rule: rule.into(),
        message: msg.into(),
        x,
        y,
    }
}

/// Interleaved on purpose so grouping has something to reorder.
fn fixture() -> Vec<ErcReportRow> {
    use ErcReportSeverity::*;
    vec![
        row(
            "a.snxsch",
            Error,
            "Unused pin",
            "Pin U1.3, floating",
            1.0,
            2.5,
        ),
        row(
            "b.snxsch",
            Warning,
            "Dangling wire endpoint",
            "say \"hi\"",
            10.0,
            20.0,
        ),
        row("a.snxsch", Info, "Net label conflict", "plain", 0.0, -1.25),
    ]
}

fn run(opts: ErcReportOptions) -> String {
    let out = ErcReportExporter.export(&ctx(), &opts).expect("export");
    String::from_utf8(out.bytes).expect("utf8")
}

fn opts(format: ErcReportFormat) -> ErcReportOptions {
    ErcReportOptions {
        format,
        violations: Some(fixture()),
        ..Default::default()
    }
}

#[test]
fn text_report_is_stable() {
    let got = run(opts(ErcReportFormat::Text));
    let want = "ERC report: 3 violation(s)\n\
a.snxsch [Error] Unused pin: Pin U1.3, floating @ (1.000, 2.500)\n\
b.snxsch [Warning] Dangling wire endpoint: say \"hi\" @ (10.000, 20.000)\n\
a.snxsch [Info] Net label conflict: plain @ (0.000, -1.250)\n";
    assert_eq!(got, want);
}

#[test]
fn csv_report_is_stable_and_escaped_by_shared_helper() {
    let got = run(opts(ErcReportFormat::Csv));
    let want = "\u{feff}Sheet,Severity,Rule,Message,X,Y\r\n\
a.snxsch,Error,Unused pin,\"Pin U1.3, floating\",1.000,2.500\r\n\
b.snxsch,Warning,Dangling wire endpoint,\"say \"\"hi\"\"\",10.000,20.000\r\n\
a.snxsch,Info,Net label conflict,plain,0.000,-1.250\r\n";
    assert_eq!(got, want);
}

#[test]
fn min_severity_filters_rows() {
    let mut o = opts(ErcReportFormat::Csv);
    o.min_severity = Some(ErcReportSeverity::Warning);
    let got = run(o);
    assert!(got.contains("Unused pin"));
    assert!(got.contains("Dangling"));
    assert!(!got.contains("Net label conflict"));
    let mut o = opts(ErcReportFormat::Text);
    o.min_severity = Some(ErcReportSeverity::Error);
    let out = ErcReportExporter.export(&ctx(), &o).unwrap();
    assert_eq!(out.row_count, 1);
}

#[test]
fn group_by_sheet_reorders_and_adds_headings() {
    let mut o = opts(ErcReportFormat::Text);
    o.group_by_sheet = true;
    let want = "ERC report: 3 violation(s)\n\
\n== a.snxsch (2) ==\n\
[Error] Unused pin: Pin U1.3, floating @ (1.000, 2.500)\n\
[Info] Net label conflict: plain @ (0.000, -1.250)\n\
\n== b.snxsch (1) ==\n\
[Warning] Dangling wire endpoint: say \"hi\" @ (10.000, 20.000)\n";
    assert_eq!(run(o), want);

    let mut o = opts(ErcReportFormat::Csv);
    o.group_by_sheet = true;
    let got = run(o);
    let lines: Vec<&str> = got.lines().collect();
    assert!(lines[2].contains("Net label conflict"), "{lines:?}");
    assert!(lines[3].starts_with("b.snxsch"));
}

#[test]
fn never_run_is_refused_not_empty() {
    let o = ErcReportOptions::default();
    let err = ErcReportExporter.export(&ctx(), &o).unwrap_err();
    assert!(matches!(err, ErcReportError::NeverRun));
}

#[test]
fn format_follows_extension() {
    use std::path::Path;
    assert_eq!(
        ErcReportFormat::from_output_path(Path::new("x.CSV")),
        ErcReportFormat::Csv
    );
    assert_eq!(
        ErcReportFormat::from_output_path(Path::new("x.txt")),
        ErcReportFormat::Text
    );
}

#[test]
fn same_file_name_in_different_directories_is_not_merged() {
    let mk = |dir: &str, msg: &str| ErcReportRow {
        sheet_key: format!("{dir}/power.snxsch"),
        sheet_name: "power.snxsch".into(),
        severity: ErcReportSeverity::Error,
        rule: "Unused pin".into(),
        message: msg.into(),
        x: 0.0,
        y: 0.0,
    };
    let mut o = ErcReportOptions {
        format: ErcReportFormat::Text,
        group_by_sheet: true,
        violations: Some(vec![mk("a", "one"), mk("b", "two"), mk("a", "three")]),
        ..Default::default()
    };
    let got = run(o.clone());
    assert!(got.contains("== a/power.snxsch (2) =="), "{got}");
    assert!(got.contains("== b/power.snxsch (1) =="), "{got}");
    o.group_by_sheet = false;
    assert!(run(o).contains("b/power.snxsch [Error]"));
}

#[test]
fn newlines_in_fields_do_not_split_a_text_row() {
    let o = ErcReportOptions {
        format: ErcReportFormat::Text,
        violations: Some(vec![row(
            "a.snxsch",
            ErcReportSeverity::Error,
            "Rule\nX",
            "line one\r\nline two",
            0.0,
            0.0,
        )]),
        ..Default::default()
    };
    let got = run(o);
    assert_eq!(got.lines().count(), 2, "{got:?}");
    assert!(got.contains(r"line one\r\nline two"), "{got:?}");
    assert!(got.contains(r"Rule\nX"), "{got:?}");
}

#[test]
fn ran_but_clean_is_a_header_only_report() {
    let o = ErcReportOptions {
        violations: Some(Vec::new()),
        ..Default::default()
    };
    let out = ErcReportExporter.export(&ctx(), &o).expect("not a refusal");
    assert_eq!(out.row_count, 0);
    assert_eq!(
        String::from_utf8(out.bytes).unwrap(),
        "ERC report: 0 violation(s)\n"
    );
}
