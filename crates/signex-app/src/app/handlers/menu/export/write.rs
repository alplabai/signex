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
            }
        },
        |outcome| Message::Export(ExportMsg::FileWritten(Box::new(outcome))),
    )
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

    pub(crate) fn handle_export_written(&mut self, outcome: ExportWriteOutcome) -> Task<Message> {
        self.end_project_write(&outcome.path);
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
