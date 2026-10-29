//! #1287: the JSON / HTML / PDF report forms.

use super::report::{ErcReportWaiver, ErcReportWaiverKind, ErcReportWaiverStatus};
use super::*;
use crate::{ExportContext, ProjectMetadata};

fn ctx() -> ExportContext {
    ExportContext {
        sheets: Vec::new(),
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    }
}

fn row(sheet: &str, severity: ErcReportSeverity, message: &str) -> ErcReportRow {
    ErcReportRow {
        sheet_key: format!("/p/{sheet}"),
        sheet_name: sheet.into(),
        severity,
        rule: "Single-pin net".into(),
        message: message.into(),
        x: 25.4,
        y: 12.7,
    }
}

fn options(format: ErcReportFormat) -> ErcReportOptions {
    use ErcReportSeverity::*;
    ErcReportOptions {
        format,
        violations: Some(vec![
            row("b.snxsch", Warning, "Net 'B' connects only one pin, R1.1"),
            row("a.snxsch", Error, "Net 'A' clash"),
            row("a.snxsch", Warning, "Net 'C' connects only one pin, R2.1"),
            row("b.snxsch", Error, "Net 'D' clash"),
        ]),
        rule_set: Some(ErcReportRuleSet {
            profile_name: Some("Signex Default".into()),
            sha256: "abc".into(),
        }),
        waivers: vec![ErcReportWaiver {
            kind: ErcReportWaiverKind::PerFinding,
            rule_id: "builtin::unused_pin".into(),
            subject: "Pin on TP1 is not connected".into(),
            reason: "Test point".into(),
            author: "A. Designer".into(),
            date: "2026-10-29".into(),
            status: ErcReportWaiverStatus::Active,
            waived_count: 1,
            severity: Some(ErcReportSeverity::Warning),
        }],
        ..ErcReportOptions::default()
    }
}

fn export(opts: &ErcReportOptions) -> Vec<u8> {
    ErcReportExporter
        .export(&ctx(), opts)
        .expect("export")
        .bytes
}

#[test]
fn the_report_groups_by_severity_then_sheet_and_counts() {
    let opts = options(ErcReportFormat::Json);
    let rows = select_rows(&opts).expect("rows");
    let r = report::build(&rows, &opts);
    assert_eq!(r.schema_version, report::ERC_REPORT_SCHEMA_VERSION);
    assert_eq!(
        (
            r.summary.errors,
            r.summary.warnings,
            r.summary.info,
            r.summary.total,
            r.summary.waived
        ),
        (2, 2, 0, 4, 1)
    );
    let shape: Vec<(&str, Vec<&str>)> = r
        .groups
        .iter()
        .map(|g| {
            (
                g.severity,
                g.sheets.iter().map(|s| s.sheet.as_str()).collect(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            ("Error", vec!["a.snxsch", "b.snxsch"]),
            ("Warning", vec!["b.snxsch", "a.snxsch"]),
        ]
    );
}

#[test]
fn json_carries_the_documented_fields() {
    let json: serde_json::Value =
        serde_json::from_slice(&export(&options(ErcReportFormat::Json))).expect("valid json");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["generator"], "signex");
    assert_eq!(json["rule_set"]["profile"], "Signex Default");
    assert_eq!(json["summary"]["total"], 4);
    let v = &json["groups"][0]["sheets"][0]["violations"][0];
    assert_eq!(v["rule"], "Single-pin net");
    assert_eq!(v["x_mm"], 25.4);
    assert_eq!(json["groups"][0]["sheets"][0]["sheet_path"], "/p/a.snxsch");
    let w = &json["waivers"][0];
    assert_eq!(w["kind"], "per_finding");
    assert_eq!(w["status"], "active");
    assert_eq!(w["reason"], "Test point");
}

#[test]
fn html_escapes_user_text_and_needs_no_external_asset() {
    let mut opts = options(ErcReportFormat::Html);
    if let Some(rows) = opts.violations.as_mut() {
        rows[0].message = "Net '<script>alert(1)</script>' & \"x\"".into();
        rows[0].sheet_name = "<img src=x onerror=1>.snxsch".into();
    }
    opts.waivers[0].reason = "</td><script>bad()</script>".into();
    let html = String::from_utf8(export(&opts)).expect("utf-8");
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("<img"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(html.contains("&amp; &quot;x&quot;"));
    for external in ["http://", "https://", "<link", "src=\"", "@import"] {
        assert!(!html.contains(external), "{external}");
    }
    assert!(html.contains("<style>"));
    assert!(html.contains("&lt;/td&gt;&lt;script&gt;bad()&lt;/script&gt;"));
    assert!(html.contains("<h2>Waivers</h2>"));
    assert!(html.find("<h2 class=\"Error\">") < html.find("<h2 class=\"Warning\">"));
}

#[test]
fn pdf_is_a_pdf_document() {
    let bytes = export(&options(ErcReportFormat::Pdf));
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 1000);
}

#[test]
fn the_format_follows_the_extension() {
    use std::path::Path;
    let f = |p: &str| ErcReportFormat::from_output_path(Path::new(p));
    assert_eq!(f("r.JSON"), ErcReportFormat::Json);
    assert_eq!(f("r.htm"), ErcReportFormat::Html);
    assert_eq!(f("r.html"), ErcReportFormat::Html);
    assert_eq!(f("r.Pdf"), ErcReportFormat::Pdf);
    assert_eq!(f("r.csv"), ErcReportFormat::Csv);
    assert_eq!(f("r.txt"), ErcReportFormat::Text);
    assert_eq!(f("r"), ErcReportFormat::Text);
}

#[test]
fn a_severity_filter_drops_lower_waivers_but_keeps_ones_of_unknown_severity() {
    let mut opts = options(ErcReportFormat::Json);
    opts.waivers.push(ErcReportWaiver {
        kind: ErcReportWaiverKind::PerNet,
        rule_id: "user::r".into(),
        subject: "net VBUS".into(),
        reason: String::new(),
        author: String::new(),
        date: String::new(),
        status: ErcReportWaiverStatus::Active,
        waived_count: 2,
        severity: None,
    });
    let all = report::build(&select_rows(&opts).expect("rows"), &opts);
    assert_eq!((all.waivers.len(), all.summary.waived), (2, 3));
    opts.min_severity = Some(ErcReportSeverity::Error);
    let errors = report::build(&select_rows(&opts).expect("rows"), &opts);
    assert_eq!(errors.summary.total, 2);
    // The Warning-level waiver drops out; the per-net one, whose severity is
    // unknown, stays listed and counted.
    assert_eq!((errors.waivers.len(), errors.summary.waived), (1, 2));
}
