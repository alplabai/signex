//! PDF emitter (#1287): the [`ErcReport`] set as flowing text through the
//! crate's own PDF writer and embedded fonts.

use super::report::ErcReport;
use super::{ErcReportError, coord};
use crate::pdf::text_document::{LineStyle, TextLine, render};

fn line(style: LineStyle, text: impl Into<String>) -> TextLine {
    TextLine {
        style,
        text: text.into(),
    }
}

pub(super) fn emit(report: &ErcReport) -> Result<Vec<u8>, ErcReportError> {
    let mut lines = vec![line(LineStyle::Title, "ERC report")];
    if report.results_stale {
        lines.push(line(LineStyle::Body, super::STALE_RESULTS_NOTICE));
    }
    if let Some(rs) = &report.rule_set {
        let profile = rs.profile.as_deref().unwrap_or("no profile");
        lines.push(line(
            LineStyle::Body,
            format!("Rule set: {profile} (sha256 {})", rs.sha256),
        ));
    }
    let s = &report.summary;
    lines.push(line(LineStyle::Heading, "Summary"));
    lines.push(line(
        LineStyle::Body,
        format!(
            "{} errors, {} warnings, {} info, {} total; {} waived",
            s.errors, s.warnings, s.info, s.total, s.waived
        ),
    ));
    for group in &report.groups {
        lines.push(line(LineStyle::Heading, group.severity));
        for sheet in &group.sheets {
            lines.push(line(
                LineStyle::Body,
                format!("{}  ({})", sheet.sheet, sheet.sheet_path),
            ));
            for v in &sheet.violations {
                lines.push(line(
                    LineStyle::Detail,
                    format!(
                        "{}: {}  @ ({}, {}) mm",
                        v.rule,
                        v.message,
                        coord(v.x_mm),
                        coord(v.y_mm)
                    ),
                ));
            }
        }
    }
    lines.push(line(LineStyle::Heading, "Waivers"));
    if report.waivers.is_empty() {
        lines.push(line(LineStyle::Body, "No waivers."));
    }
    for w in &report.waivers {
        lines.push(line(
            LineStyle::Body,
            format!("{}: {} [{}]", w.rule_id, w.subject, w.status.label()),
        ));
        lines.push(line(
            LineStyle::Detail,
            format!(
                "Reason: {}  Author: {}  Date: {}  Hidden: {}",
                w.reason, w.author, w.date, w.waived_count
            ),
        ));
    }
    render(&lines).map_err(|e| ErcReportError::Pdf(e.to_string()))
}
