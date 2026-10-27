//! File ▸ Export ▸ ERC Report — plain text / CSV of the cached ERC results.
//!
//! Rows come from `build_erc_diagnostic_entries`, the Messages panel's own row
//! model, so the file matches the panel in count, order, severity and message.
//! ERC is never run from here: a project whose ERC has not been run is refused
//! with a visible message, and nothing is written until the user has picked a
//! path in the save dialog.

use std::path::PathBuf;

use iced::Task;
use signex_output::{
    ErcReportExporter, ErcReportFormat, ErcReportOptions, ErcReportRow, ErcReportSeverity,
    ExportContext, Exporter, ProjectMetadata,
};

use super::super::super::super::*;
use crate::panels::{ErcDiagnosticEntry, ErcSeverityLite};

#[cfg(test)]
#[path = "erc_report_tests.rs"]
mod tests;

fn report_row(entry: &ErcDiagnosticEntry) -> ErcReportRow {
    ErcReportRow {
        sheet_key: entry.sheet_path.display().to_string(),
        sheet_name: entry.sheet_name.clone(),
        severity: match entry.severity {
            ErcSeverityLite::Error => ErcReportSeverity::Error,
            ErcSeverityLite::Warning => ErcReportSeverity::Warning,
            ErcSeverityLite::Info => ErcReportSeverity::Info,
        },
        rule: entry.rule_label.to_string(),
        message: entry.message.clone(),
        x: entry.world_x,
        y: entry.world_y,
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
        self.erc_has_been_run().then(|| {
            self.build_erc_diagnostic_entries()
                .iter()
                .map(report_row)
                .collect()
        })
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
                    .add_filter("Text (.txt)", &["txt"])
                    .add_filter("CSV (.csv)", &["csv"])
                    .set_file_name("erc-report.txt")
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |path| match path {
                Some(path) => Message::Export(ExportMsg::ErcReportPathChosen(Ok(path))),
                None => Message::Noop,
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
        let opts = ErcReportOptions {
            format: ErcReportFormat::from_output_path(&path),
            violations: self.erc_report_rows(),
            ..ErcReportOptions::default()
        };
        // The exporter formats pre-derived rows and ignores the context.
        let ctx = ExportContext {
            sheets: Vec::new(),
            metadata: ProjectMetadata::default(),
            netlist: None,
            netlist_sheets: Vec::new(),
            current_sheet: None,
            netlist_placements: Vec::new(),
        };
        let output = match ErcReportExporter.export(&ctx, &opts) {
            Ok(o) => o,
            Err(e) => {
                self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                    format!("ERC report export failed: {e}"),
                ));
                return Task::none();
            }
        };
        // The shared export write holds a folder rename until the report
        // has landed.
        let success_log = format!("Wrote ERC report {}", path.display());
        self.write_export(
            path,
            output.bytes,
            "ERC report",
            success_log,
            None,
            Vec::new(),
        )
    }
}
