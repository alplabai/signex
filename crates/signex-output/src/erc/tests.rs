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
