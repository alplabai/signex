//! File ▸ Export ▸ ERC Report — text, CSV, JSON, HTML or PDF of the cached
//! ERC results, with the project's waivers (#1287).
//!
//! Rows come from `build_erc_diagnostic_entries`, the Messages panel's own row
//! model, so the file matches the panel in count, order, severity and message.
//! ERC is never run from here: a project whose ERC has not been run is refused
//! with a visible message, and nothing is written until the user has picked a
//! path in the save dialog.

use std::path::PathBuf;

use iced::Task;
use signex_output::erc_report::{ErcReportWaiver, ErcReportWaiverKind, ErcReportWaiverStatus};
use signex_output::{
    ErcReportExporter, ErcReportFormat, ErcReportOptions, ErcReportRow, ErcReportSeverity,
    ExportContext, Exporter, ProjectMetadata,
};

use super::super::super::super::*;
use crate::app::contracts::ExportWriteOutcome;
use crate::panels::{ErcDiagnosticEntry, ErcSeverityLite};

#[cfg(test)]
#[path = "erc_report_tests.rs"]
mod tests;

/// A per-finding waiver as the report lists it.
fn report_waiver(w: &crate::panels::ErcViolationWaiverEntry) -> ErcReportWaiver {
    use crate::panels::ErcWaiverStatus as S;
    let (status, waived_count) = match w.status {
        S::Waiving(n) => (ErcReportWaiverStatus::Active, n),
        S::Lapsed => (ErcReportWaiverStatus::Lapsed, 0),
        S::Unmatched => (ErcReportWaiverStatus::Unmatched, 0),
        S::Incomplete => (ErcReportWaiverStatus::Incomplete, 0),
    };
    ErcReportWaiver {
        kind: ErcReportWaiverKind::PerFinding,
        rule_id: w.rule_id.clone(),
        subject: w.message.clone(),
        reason: w.reason.clone(),
        author: w.author.clone(),
        date: w.date.clone(),
        status,
        waived_count,
        severity: w.severity.map(lite_severity),
    }
}

fn lite_severity(s: ErcSeverityLite) -> ErcReportSeverity {
    match s {
        ErcSeverityLite::Error => ErcReportSeverity::Error,
        ErcSeverityLite::Warning => ErcReportSeverity::Warning,
        ErcSeverityLite::Info => ErcReportSeverity::Info,
    }
}

/// A per-net waiver (#1581), which records no reason or author.
fn report_net_waiver(w: &crate::panels::ErcWaiverEntry) -> ErcReportWaiver {
    ErcReportWaiver {
        kind: ErcReportWaiverKind::PerNet,
        rule_id: w.rule_id.clone(),
        subject: format!("net {}", w.net),
        reason: String::new(),
        author: String::new(),
        date: String::new(),
        status: if w.hidden > 0 {
            ErcReportWaiverStatus::Active
        } else {
            ErcReportWaiverStatus::Unmatched
        },
        waived_count: w.hidden,
        severity: None,
    }
}

/// `path` relative to the project folder when it lies inside it, so a report
/// does not carry the local folder layout; otherwise the path as is.
fn report_sheet_path(path: &std::path::Path, project_dir: Option<&std::path::Path>) -> String {
    project_dir
        .and_then(|dir| path.strip_prefix(dir).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn report_row(entry: &ErcDiagnosticEntry, project_dir: Option<&std::path::Path>) -> ErcReportRow {
    ErcReportRow {
        sheet_key: report_sheet_path(&entry.sheet_path, project_dir),
        sheet_name: entry.sheet_name.clone(),
        severity: lite_severity(entry.severity),
        rule: entry.rule_label.to_string(),
        message: entry.message.clone(),
        x: entry.world_x,
        y: entry.world_y,
    }
}

/// Format the report and write it, both off the UI thread: a PDF or a large
/// HTML report takes long enough to stall `update`. The outcome comes back as
/// `ExportMsg::FileWritten`, like every other export write.
fn export_erc_report_task(opts: ErcReportOptions, path: PathBuf) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || build_and_write_erc_report(&opts, &path))
                .await
                .unwrap_or_else(|e| ExportWriteOutcome {
                    path: target,
                    result: Err(format!("ERC report export failed: {e}")),
                    success_log: String::new(),
                    advisory: None,
                    warnings: Vec::new(),
                })
        },
        |outcome| Message::Export(ExportMsg::FileWritten(Box::new(outcome))),
    )
}

/// Format `opts` and write the bytes atomically to `path`.
fn build_and_write_erc_report(
    opts: &ErcReportOptions,
    path: &std::path::Path,
) -> ExportWriteOutcome {
    // The exporter formats pre-derived rows and ignores the context.
    let ctx = ExportContext {
        sheets: Vec::new(),
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    };
    let result = ErcReportExporter
        .export(&ctx, opts)
        .map_err(|e| format!("ERC report export failed: {e}"))
        .and_then(|output| {
            signex_types::atomic_io::atomic_write(path, &output.bytes)
                .map_err(|e| format!("Could not write ERC report to {}:\n{e}", path.display()))
        });
    ExportWriteOutcome {
        path: path.to_path_buf(),
        result,
        success_log: format!("Wrote ERC report {}", path.display()),
        advisory: None,
        warnings: Vec::new(),
    }
}

impl Signex {
    /// ERC has run when the per-sheet cache has an entry (a clean sheet is an
    /// entry with no violations); an empty cache means it never has.
    fn erc_has_been_run(&self) -> bool {
        !self.ui_state.erc_violations_by_path.is_empty()
    }

    /// The Messages panel's rows, or `None` when ERC has never been run.
    fn erc_report_rows(&self) -> Option<Vec<ErcReportRow>> {
        let project_dir = self
            .document_state
            .active_document_project()
            .map(|p| p.dir().to_path_buf());
        self.erc_has_been_run().then(|| {
            self.build_erc_diagnostic_entries()
                .iter()
                .map(|e| report_row(e, project_dir.as_deref()))
                .collect()
        })
    }

    /// The report's content: the cached rows, and the rule set the newest run
    /// used so the header can name the profile and hash (#1085).
    pub(crate) fn erc_report_options(&self, format: ErcReportFormat) -> ErcReportOptions {
        ErcReportOptions {
            format,
            violations: self.erc_report_rows(),
            rule_set: self.ui_state.erc_run_rule_set.clone(),
            results_stale: self.ui_state.erc_results_stale,
            waivers: self
                .erc_violation_waiver_entries()
                .iter()
                .map(report_waiver)
                .chain(self.erc_waiver_entries().iter().map(report_net_waiver))
                .collect(),
            ..ErcReportOptions::default()
        }
    }

    fn refuse_erc_report_before_run(&mut self) {
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(format!(
            "Cannot export the ERC report: ERC has not been run. \
                 Run ERC ({}) first, then export.",
            crate::menu_bar::run_erc_menu_path()
        )));
    }

    /// Menu entry: refuse if ERC never ran, otherwise ask where to save.
    pub(crate) fn handle_erc_report_requested(&mut self) -> Task<Message> {
        if !self.erc_has_been_run() {
            self.refuse_erc_report_before_run();
            return Task::none();
        }
        Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Export ERC Report")
                    .add_filter("HTML (.html)", &["html", "htm"])
                    .add_filter("PDF (.pdf)", &["pdf"])
                    .add_filter("JSON (.json)", &["json"])
                    .add_filter("CSV (.csv)", &["csv"])
                    .add_filter("Text (.txt)", &["txt"])
                    .set_file_name("erc-report.html")
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |path| match path {
                Some(path) => Message::Export(ExportMsg::ErcReportPathChosen(Ok(path))),
                None => Message::noop(),
            },
        )
    }

    /// The user picked a path: format the cached rows and write them off the
    /// update thread. The format follows the saved-as extension.
    pub(crate) fn handle_erc_report_path_chosen(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let path = match result {
            Ok(p) => p,
            Err(e) => {
                log::info!("ERC report export cancelled: {e}");
                return Task::none();
            }
        };
        // Before formatting, so a moving project gets that reason.
        if self.refuse_with_notice_while_project_moves(
            &path,
            "export to",
            crate::app::state::ErrorNotice::export,
        ) {
            return Task::none();
        }
        // Checked here, not left to the exporter, so the refusal shows now
        // rather than as a failed write.
        if !self.erc_has_been_run() {
            self.refuse_erc_report_before_run();
            return Task::none();
        }
        let opts = self.erc_report_options(ErcReportFormat::from_output_path(&path));
        if opts.results_stale {
            crate::diagnostics::log_warning(
                "The ERC report is written from results that may be out of date: the sheet changed after the last run. Run ERC again for a current report.",
            );
        }
        // Held like any export write, so a folder rename waits for it.
        self.begin_project_write(&path);
        export_erc_report_task(opts, path)
    }
}
