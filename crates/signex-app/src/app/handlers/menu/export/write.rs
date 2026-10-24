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
pub(super) fn write_export_task(
    path: PathBuf,
    bytes: Vec<u8>,
    what: &'static str,
    success_log: String,
    advisory: Option<ErrorNotice>,
    warnings: Vec<String>,
) -> Task<Message> {
    Task::perform(
        async move {
            let result = tokio::task::spawn_blocking(move || {
                signex_types::atomic_io::atomic_write(&path, &bytes)
                    .map_err(|e| format!("Could not write {what} to {}:\n{e}", path.display()))
            })
            .await
            .unwrap_or_else(|e| Err(format!("Could not write {what}: {e}")));
            ExportWriteOutcome {
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
    pub(crate) fn handle_export_written(&mut self, outcome: ExportWriteOutcome) -> Task<Message> {
        match outcome.result {
            Ok(()) => {
                log::info!("{}", outcome.success_log);
                if let Some(notice) = outcome.advisory
                    && self.document_state.error_notice.is_none()
                {
                    self.document_state.error_notice = Some(notice);
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
