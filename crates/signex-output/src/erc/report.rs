//! The structured ERC report (#1287) behind the JSON, HTML and PDF forms:
//! one pure builder, so the three agree and the shape is unit-testable. The
//! JSON form is this model serialised; its schema is `docs/ERC_REPORT.md`.

use serde::Serialize;

use super::{ErcReportOptions, ErcReportRow, ErcReportRuleSet, ErcReportSeverity};

/// Version of the JSON report shape. Bump on any breaking change.
pub const ERC_REPORT_SCHEMA_VERSION: u32 = 1;

/// A stored waiver as the report lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ErcReportWaiver {
    /// `per_finding` (#1285) or `per_net` (#1581).
    pub kind: ErcReportWaiverKind,
    pub rule_id: String,
    /// The finding's message when waived, or the net name for a net waiver.
    pub subject: String,
    /// Empty for a net waiver, which records none.
    pub reason: String,
    pub author: String,
    pub date: String,
    pub status: ErcReportWaiverStatus,
    /// Findings it hid in the run the report was made from.
    pub waived_count: usize,
    /// Severity of the finding it hid, when known. A report filtered by a
    /// minimum severity lists only waivers whose finding passes the filter.
    pub severity: Option<ErcReportSeverity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErcReportWaiverKind {
    PerFinding,
    PerNet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErcReportWaiverStatus {
    /// It hid at least one finding.
    Active,
    /// An involved object changed, so its finding is reported again.
    Lapsed,
    /// It matched no finding.
    Unmatched,
    /// No reason or author; never honoured.
    Incomplete,
}

impl ErcReportWaiverStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Lapsed => "lapsed",
            Self::Unmatched => "matches no finding",
            Self::Incomplete => "incomplete (ignored)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ErcReport {
    pub schema_version: u32,
    pub generator: &'static str,
    pub rule_set: Option<ReportRuleSet>,
    pub summary: ReportSummary,
    /// Severity groups, Error first, each with its sheets in first-seen order.
    pub groups: Vec<SeverityGroup>,
    pub waivers: Vec<ErcReportWaiver>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReportRuleSet {
    pub profile: Option<String>,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReportSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub total: usize,
    /// Findings hidden by the listed waivers (not in `groups`); the same
    /// minimum-severity filter as the findings applies.
    pub waived: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeverityGroup {
    pub severity: &'static str,
    pub sheets: Vec<SheetGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SheetGroup {
    pub sheet: String,
    /// The sheet's path as the caller gives it; the app passes it relative
    /// to the project folder.
    pub sheet_path: String,
    pub violations: Vec<ReportViolation>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReportViolation {
    pub rule: String,
    pub message: String,
    pub x_mm: f64,
    pub y_mm: f64,
}

const SEVERITY_ORDER: [ErcReportSeverity; 3] = [
    ErcReportSeverity::Error,
    ErcReportSeverity::Warning,
    ErcReportSeverity::Info,
];

/// Build the report from the rows the caller selected (filtered, in panel
/// order) and the options' rule set and waivers.
pub fn build(rows: &[&ErcReportRow], opts: &ErcReportOptions) -> ErcReport {
    let count = |sev| rows.iter().filter(|r| r.severity == sev).count();
    let summary = ReportSummary {
        errors: count(ErcReportSeverity::Error),
        warnings: count(ErcReportSeverity::Warning),
        info: count(ErcReportSeverity::Info),
        total: rows.len(),
        waived: 0,
    };
    let waivers: Vec<ErcReportWaiver> = opts
        .waivers
        .iter()
        .filter(|w| match (opts.min_severity, w.severity) {
            (None, _) => true,
            (Some(min), Some(sev)) => sev >= min,
            // Severity unknown (per-net, lapsed or unmatched): keep it, so a
            // filtered report never under-states what was waived.
            (Some(_), None) => true,
        })
        .cloned()
        .collect();
    let summary = ReportSummary {
        waived: waivers.iter().map(|w| w.waived_count).sum(),
        ..summary
    };
    let groups = SEVERITY_ORDER
        .iter()
        .filter_map(|&sev| {
            let of_sev: Vec<&ErcReportRow> =
                rows.iter().copied().filter(|r| r.severity == sev).collect();
            (!of_sev.is_empty()).then(|| SeverityGroup {
                severity: sev.label(),
                sheets: by_sheet(&of_sev),
            })
        })
        .collect();
    ErcReport {
        schema_version: ERC_REPORT_SCHEMA_VERSION,
        generator: "signex",
        rule_set: opts.rule_set.as_ref().map(rule_set),
        summary,
        groups,
        waivers,
    }
}

fn rule_set(r: &ErcReportRuleSet) -> ReportRuleSet {
    ReportRuleSet {
        profile: r.profile_name.clone(),
        sha256: r.sha256.clone(),
    }
}

fn by_sheet(rows: &[&ErcReportRow]) -> Vec<SheetGroup> {
    let mut groups: Vec<SheetGroup> = Vec::new();
    for r in rows {
        let violation = ReportViolation {
            rule: r.rule.clone(),
            message: r.message.clone(),
            x_mm: r.x,
            y_mm: r.y,
        };
        match groups.iter_mut().find(|g| g.sheet_path == r.sheet_key) {
            Some(g) => g.violations.push(violation),
            None => groups.push(SheetGroup {
                sheet: r.sheet_name.clone(),
                sheet_path: r.sheet_key.clone(),
                violations: vec![violation],
            }),
        }
    }
    groups
}
