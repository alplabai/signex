//! Self-contained HTML emitter (#1287): inline CSS, no scripts, no external
//! assets. Every user-supplied string (messages carry net names and
//! designators, waivers carry free text) goes through [`escape`].

use super::coord;
use super::report::{ErcReport, SeverityGroup};

const STYLE: &str = "body{font-family:system-ui,sans-serif;margin:2em;color:#222}\
h1{font-size:1.4em}h2{font-size:1.15em;margin-top:1.5em}h3{font-size:1em;color:#555}\
table{border-collapse:collapse;width:100%;margin:.5em 0}\
th,td{border:1px solid #ccc;padding:.3em .5em;text-align:left;vertical-align:top}\
th{background:#f3f3f3}.Error{color:#b00020}.Warning{color:#a15c00}.Info{color:#0a5bb5}\
.summary td{text-align:right}.muted{color:#777}";

/// Escape `s` for HTML text and double-quoted attribute content.
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

pub(super) fn emit(report: &ErcReport) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<title>ERC report</title>\n<style>");
    out.push_str(STYLE);
    out.push_str("</style>\n</head>\n<body>\n<h1>ERC report</h1>\n");
    if report.results_stale {
        out.push_str(&format!(
            "<p class=\"muted\"><strong>{}</strong></p>\n",
            escape(super::STALE_RESULTS_NOTICE)
        ));
    }
    if let Some(rs) = &report.rule_set {
        let profile = rs.profile.as_deref().unwrap_or("no profile");
        out.push_str(&format!(
            "<p class=\"muted\">Rule set: {} (sha256 {})</p>\n",
            escape(profile),
            escape(&rs.sha256)
        ));
    }
    summary(report, &mut out);
    for group in &report.groups {
        severity_group(group, &mut out);
    }
    waivers(report, &mut out);
    out.push_str("</body>\n</html>\n");
    out.into_bytes()
}

fn summary(report: &ErcReport, out: &mut String) {
    let s = &report.summary;
    out.push_str("<h2>Summary</h2>\n<table class=\"summary\">\n");
    for (label, n) in [
        ("Errors", s.errors),
        ("Warnings", s.warnings),
        ("Info", s.info),
        ("Total", s.total),
        ("Waived", s.waived),
    ] {
        out.push_str(&format!("<tr><th>{label}</th><td>{n}</td></tr>\n"));
    }
    out.push_str("</table>\n");
}

fn severity_group(group: &SeverityGroup, out: &mut String) {
    let sev = group.severity;
    out.push_str(&format!("<h2 class=\"{sev}\">{sev}</h2>\n"));
    for sheet in &group.sheets {
        out.push_str(&format!(
            "<h3>{} <span class=\"muted\">{}</span></h3>\n",
            escape(&sheet.sheet),
            escape(&sheet.sheet_path)
        ));
        out.push_str("<table>\n<tr><th>Rule</th><th>Message</th><th>Location (mm)</th></tr>\n");
        for v in &sheet.violations {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>({}, {})</td></tr>\n",
                escape(&v.rule),
                escape(&v.message),
                coord(v.x_mm),
                coord(v.y_mm)
            ));
        }
        out.push_str("</table>\n");
    }
}

fn waivers(report: &ErcReport, out: &mut String) {
    out.push_str("<h2>Waivers</h2>\n");
    if report.waivers.is_empty() {
        out.push_str("<p class=\"muted\">No waivers.</p>\n");
        return;
    }
    out.push_str(
        "<table>\n<tr><th>Rule</th><th>Finding</th><th>Reason</th><th>Author</th>\
         <th>Date</th><th>Status</th><th>Hidden</th></tr>\n",
    );
    for w in &report.waivers {
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape(&w.rule_id),
            escape(&w.subject),
            escape(&w.reason),
            escape(&w.author),
            escape(&w.date),
            w.status.label(),
            w.waived_count
        ));
    }
    out.push_str("</table>\n");
}
