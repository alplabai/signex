//! Off-thread export file writes (#859): the exporters build their bytes in
//! `update`, but the disk write runs on a blocking task and its outcome comes
//! back as `ExportMsg::FileWritten`.

use std::path::PathBuf;

use iced::Task;

use super::super::super::super::*;
use crate::app::contracts::ExportWriteOutcome;
use crate::app::state::ErrorNotice;

/// Write `bytes` to `path` crash-safely off the UI thread. `what` names the
/// deliverable in the failure text ("PDF", "netlist", "BOM").
fn write_export_task(
    path: PathBuf,
    bytes: Vec<u8>,
    what: &'static str,
    success_log: String,
    advisory: Option<ErrorNotice>,
    warnings: Vec<String>,
) -> Task<Message> {
    Task::perform(
        async move {
            let target = path.clone();
            let result = tokio::task::spawn_blocking(move || {
                signex_types::atomic_io::atomic_write(&path, &bytes)
                    .map_err(|e| format!("Could not write {what} to {}:\n{e}", path.display()))
            })
            .await
            .unwrap_or_else(|e| Err(format!("Could not write {what}: {e}")));
            ExportWriteOutcome {
                path: target,
                result,
                success_log,
                advisory,
                warnings,
                report_in_preview: false,
            }
        },
        |outcome| Message::Export(ExportMsg::FileWritten(Box::new(outcome))),
    )
}

/// Build the PDF and write it to `path`, both off the UI thread: laying out
/// and encoding a many-page document takes long enough to freeze the window
/// if it ran inside `update`. The outcome comes back as
/// `ExportMsg::FileWritten`, like every other export write.
fn export_pdf_task(
    ctx: signex_output::ExportContext,
    options: signex_output::PdfOptions,
    path: PathBuf,
) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || build_and_write_pdf(&ctx, &options, &path))
                .await
                .unwrap_or_else(|e| ExportWriteOutcome {
                    path: target,
                    result: Err(format!("PDF export failed: {e}")),
                    success_log: String::new(),
                    advisory: None,
                    warnings: Vec::new(),
                    report_in_preview: true,
                })
        },
        |outcome| Message::Export(ExportMsg::FileWritten(Box::new(outcome))),
    )
}

fn build_and_write_pdf(
    ctx: &signex_output::ExportContext,
    options: &signex_output::PdfOptions,
    path: &std::path::Path,
) -> ExportWriteOutcome {
    use signex_output::Exporter;
    let failed = |reason: String| ExportWriteOutcome {
        path: path.to_path_buf(),
        result: Err(reason),
        success_log: String::new(),
        advisory: None,
        warnings: Vec::new(),
        report_in_preview: true,
    };
    let output = match signex_output::PdfExporter.export(ctx, options) {
        Ok(output) => output,
        Err(e) => return failed(format!("PDF export failed: {e}")),
    };
    let success_log = format!(
        "Wrote {} ({} page(s), {} bytes)",
        path.display(),
        output.page_count,
        output.bytes.len(),
    );
    let result = signex_types::atomic_io::atomic_write(path, &output.bytes)
        .map_err(|e| format!("Could not write PDF to {}:\n{e}", path.display()));
    ExportWriteOutcome {
        path: path.to_path_buf(),
        result,
        success_log,
        advisory: None,
        warnings: super::uncovered_chars_warning(&output.uncovered_chars)
            .into_iter()
            .collect(),
        report_in_preview: true,
    }
}

/// Run `task` and every task its messages produce to completion, feeding
/// each message back through `update` (what the iced runtime does).
#[cfg(test)]
pub(crate) fn settle(app: &mut Signex, task: Task<Message>) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = futures::StreamExt::next(&mut stream).await {
                if let iced_runtime::Action::Output(message) = action {
                    messages.push(message);
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}

impl Signex {
    /// [`write_export_task`], unless `path` lies in a project whose folder is
    /// being renamed: the write would run during the move and either land in
    /// the old folder before the re-key or recreate it as an orphan, so the
    /// export is refused with a notice instead.
    pub(crate) fn write_export(
        &mut self,
        path: PathBuf,
        bytes: Vec<u8>,
        what: &'static str,
        success_log: String,
        advisory: Option<ErrorNotice>,
        warnings: Vec<String>,
    ) -> Task<Message> {
        if self.refuse_with_notice_while_project_moves(&path, "export to", ErrorNotice::export) {
            return Task::none();
        }
        self.begin_project_write(&path);
        write_export_task(path, bytes, what, success_log, advisory, warnings)
    }

    /// [`export_pdf_task`], refused like [`Self::write_export`] while the
    /// target's project folder is being renamed.
    pub(crate) fn export_pdf(
        &mut self,
        ctx: signex_output::ExportContext,
        options: signex_output::PdfOptions,
        path: PathBuf,
    ) -> Task<Message> {
        if self.refuse_with_notice_while_project_moves(&path, "export to", ErrorNotice::export) {
            return Task::none();
        }
        // Held like any export write, so a folder rename waits for it.
        self.begin_project_write(&path);
        export_pdf_task(ctx, options, path)
    }

    pub(crate) fn handle_export_written(&mut self, outcome: ExportWriteOutcome) -> Task<Message> {
        self.end_project_write(&outcome.path);
        if outcome.report_in_preview
            && let Some(preview) = self.document_state.preview.as_mut()
        {
            preview.export_status = Some(match &outcome.result {
                Ok(()) => format!("Exported {}", outcome.path.display()),
                Err(e) => format!("Export failed: {e}"),
            });
        }
        match outcome.result {
            Ok(()) => {
                log::info!("{}", outcome.success_log);
                if let Some(notice) = outcome.advisory {
                    if self.document_state.error_notice.is_none() {
                        self.document_state.error_notice = Some(notice);
                    } else {
                        crate::diagnostics::log_warning(format!(
                            "Export advisory dropped, a notice is already showing: {}",
                            notice.detail
                        ));
                    }
                }
                for warning in outcome.warnings {
                    crate::diagnostics::log_warning(warning);
                }
            }
            Err(e) => {
                self.document_state.error_notice = Some(ErrorNotice::export(e));
            }
        }
        Task::none()
    }
}
