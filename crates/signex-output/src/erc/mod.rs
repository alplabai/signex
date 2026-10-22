//! ERC report exporter — plain text and CSV.
//!
//! The exporter formats violations the caller already has; it never runs ERC
//! (the same contract the netlist exporter keeps with its pre-derived
//! netlist). Rows mirror the Messages panel: same order, same content.

mod csv;
mod text;

#[cfg(test)]
mod tests;

use thiserror::Error;

use crate::{ExportContext, Exporter};

/// Severity of one report row, ordered `Info < Warning < Error` so a minimum
/// severity filter is a plain comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErcReportSeverity {
    Info,
    Warning,
    Error,
}

impl ErcReportSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

/// One violation as it appears in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct ErcReportRow {
    /// Identity of the sheet (its full path). Grouping keys on this, so two
    /// sheets that share a file name in different directories stay apart.
    pub sheet_key: String,
    /// Bare display name of the sheet (its file name).
    pub sheet_name: String,
    pub severity: ErcReportSeverity,
    pub rule: String,
    pub message: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErcReportFormat {
    #[default]
    Text,
    Csv,
}

impl ErcReportFormat {
    /// CSV when the chosen path ends in `.csv` (any case), text otherwise.
    pub fn from_output_path(path: &std::path::Path) -> Self {
        let is_csv = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
        if is_csv { Self::Csv } else { Self::Text }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ErcReportOptions {
    pub format: ErcReportFormat,
    /// Rows below this severity are dropped. `None` keeps everything.
    pub min_severity: Option<ErcReportSeverity>,
    /// Gather rows by sheet (first-appearance order, row order preserved
    /// within a sheet). Text output also gets a heading per sheet.
    pub group_by_sheet: bool,
    /// The cached ERC results in Messages-panel order. `None` means ERC has
    /// never been run — the exporter refuses rather than emit an empty file.
    pub violations: Option<Vec<ErcReportRow>>,
}

#[derive(Debug, Clone)]
pub struct ErcReportOutput {
    pub bytes: Vec<u8>,
    /// Rows written after filtering.
    pub row_count: usize,
}

#[derive(Debug, Error)]
pub enum ErcReportError {
    #[error("ERC has not been run — run ERC first, then export the report")]
    NeverRun,
    #[error("csv: {0}")]
    Csv(String),
}

pub struct ErcReportExporter;

impl Exporter for ErcReportExporter {
    type Options = ErcReportOptions;
    type Output = ErcReportOutput;
    type Error = ErcReportError;

    fn export(
        &self,
        _ctx: &ExportContext,
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        let rows = select_rows(opts).ok_or(ErcReportError::NeverRun)?;
        let labels = sheet_labels(&rows);
        let bytes = match opts.format {
            ErcReportFormat::Text => text::emit(&rows, &labels, opts.group_by_sheet),
            ErcReportFormat::Csv => csv::emit(&rows, &labels)?,
        };
        Ok(ErcReportOutput {
            bytes,
            row_count: rows.len(),
        })
    }
}

/// Filter by severity, then (optionally) gather by sheet.
fn select_rows(opts: &ErcReportOptions) -> Option<Vec<&ErcReportRow>> {
    let all = opts.violations.as_ref()?;
    let mut rows: Vec<&ErcReportRow> = all
        .iter()
        .filter(|r| opts.min_severity.is_none_or(|min| r.severity >= min))
        .collect();
    if opts.group_by_sheet {
        let mut order: Vec<&str> = Vec::new();
        for r in &rows {
            if !order.contains(&r.sheet_key.as_str()) {
                order.push(&r.sheet_key);
            }
        }
        // Stable sort keeps in-sheet order.
        rows.sort_by_key(|r| order.iter().position(|s| *s == r.sheet_key));
    }
    Some(rows)
}

/// Display name per row, parallel to `rows`. The bare file name is used unless
/// two different sheets share it, in which case the full sheet path is shown
/// so the reader can tell them apart.
fn sheet_labels(rows: &[&ErcReportRow]) -> Vec<String> {
    rows.iter()
        .map(|r| {
            let clash = rows
                .iter()
                .any(|o| o.sheet_name == r.sheet_name && o.sheet_key != r.sheet_key);
            if clash {
                r.sheet_key.clone()
            } else {
                r.sheet_name.clone()
            }
        })
        .collect()
}

pub(crate) const COLUMNS: [&str; 6] = ["Sheet", "Severity", "Rule", "Message", "X", "Y"];

pub(crate) fn coord(v: f64) -> String {
    format!("{v:.3}")
}
