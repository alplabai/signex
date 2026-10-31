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
                    .map_err(|e| format!("{e}: could not write {what} to {}", path.display()))
            })
            .await
            .unwrap_or_else(|e| Err(format!("Could not write {what}: {e}")));
            ExportWriteOutcome {
                path: target,
                result,
                success_log,
                advisory,
                warnings,
                preview_source: None,
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
    source: PathBuf,
) -> Task<Message> {
    let target = path.clone();
    let failed_source = source.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || build_and_write_pdf(&ctx, &options, &path, &source))
                .await
                .unwrap_or_else(|e| ExportWriteOutcome {
                    path: target,
                    result: Err(format!("PDF export failed: {e}")),
                    success_log: String::new(),
                    advisory: None,
                    warnings: Vec::new(),
                    preview_source: Some(failed_source),
                })
        },
        |outcome| Message::Export(ExportMsg::FileWritten(Box::new(outcome))),
    )
}

fn build_and_write_pdf(
    ctx: &signex_output::ExportContext,
    options: &signex_output::PdfOptions,
    path: &std::path::Path,
    source: &std::path::Path,
) -> ExportWriteOutcome {
    use signex_output::Exporter;
    let failed = |reason: String| ExportWriteOutcome {
        path: path.to_path_buf(),
        result: Err(reason),
        success_log: String::new(),
        advisory: None,
        warnings: Vec::new(),
        preview_source: Some(source.to_path_buf()),
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
        .map_err(|e| format!("{e}: could not write PDF to {}", path.display()));
    ExportWriteOutcome {
        path: path.to_path_buf(),
        result,
        success_log,
        advisory: None,
        warnings: super::uncovered_chars_warning(&output.uncovered_chars)
            .into_iter()
            .collect(),
        preview_source: Some(source.to_path_buf()),
    }
}

/// The Export window's one-line outcome for a PDF export.
pub(crate) fn export_status_line(path: &std::path::Path, result: &Result<(), String>) -> String {
    match result {
        Ok(()) => format!("Exported {}", elide_middle(&path.display().to_string())),
        Err(e) => format!(
            "Export failed: {}",
            elide_middle(&e.split_whitespace().collect::<Vec<_>>().join(" "))
        ),
    }
}

/// Longest text a status line shows before its middle is elided.
const STATUS_TEXT_MAX_CHARS: usize = 72;

/// `text` shortened to [`STATUS_TEXT_MAX_CHARS`] by replacing its middle with
/// an ellipsis, so a path keeps its root and its file name.
fn elide_middle(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= STATUS_TEXT_MAX_CHARS {
        return text.to_string();
    }
    let tail = STATUS_TEXT_MAX_CHARS * 2 / 3;
    let head = STATUS_TEXT_MAX_CHARS - tail - 1;
    let head: String = chars[..head].iter().collect();
    let tail: String = chars[chars.len() - tail..].iter().collect();
    format!("{head}…{tail}")
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
        source: PathBuf,
    ) -> Task<Message> {
        if self.refuse_with_notice_while_project_moves(&path, "export to", ErrorNotice::export) {
            return Task::none();
        }
        // Held like any export write, so a folder rename waits for it.
        self.begin_project_write(&path);
        export_pdf_task(ctx, options, path, source)
    }

    /// An export failed for real (building or writing the file): the notice
    /// card plus an error line in the Messages panel, so the failure outlives
    /// the card.
    pub(crate) fn report_export_failure(&mut self, detail: impl Into<String>) {
        let detail = detail.into();
        log::error!("{detail}");
        self.document_state.error_notice = Some(ErrorNotice::export(detail));
    }

    /// An export was refused up front (a precondition or an input of the
    /// user's): the notice card plus a warning line in the Messages panel.
    pub(crate) fn refuse_export(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        crate::diagnostics::log_warning(&reason);
        self.document_state.error_notice = Some(ErrorNotice::export(reason));
    }

    pub(crate) fn handle_export_written(&mut self, outcome: ExportWriteOutcome) -> Task<Message> {
        self.end_project_write(&outcome.path);
        if let Some(preview) = self.document_state.preview.as_mut()
            && outcome.preview_source.as_ref() == Some(&preview.source)
        {
            preview.export_status = Some(export_status_line(&outcome.path, &outcome.result));
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
                self.report_export_failure(e);
            }
        }
        Task::none()
    }
}
