//! File ▸ Import ▸ Altium Project: run the converter off-thread, then open
//! what it wrote. The process contract lives in `crate::altium_import`.

use std::path::PathBuf;

use super::super::super::*;
use crate::altium_import::{self, ImportOutcome};
use crate::app::state::ErrorNotice;

impl Signex {
    pub(crate) fn handle_altium_import_picked(
        &mut self,
        path: Option<PathBuf>,
    ) -> iced::Task<Message> {
        let Some(input) = path else {
            return iced::Task::none();
        };
        let Some(binary) = altium_import::locate() else {
            self.document_state.error_notice =
                Some(ErrorNotice::import(altium_import::not_installed_message()));
            return iced::Task::none();
        };
        let out_dir = altium_import::output_dir_for(&input);
        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || altium_import::run(&binary, &input, &out_dir))
                    .await
                    .unwrap_or_else(|error| Err(format!("Altium import did not complete: {error}")))
            },
            |result| Message::File(FileMsg::AltiumImportFinished(result)),
        )
    }

    pub(crate) fn handle_altium_import_finished(
        &mut self,
        result: Result<ImportOutcome, String>,
    ) -> iced::Task<Message> {
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(message) => {
                crate::diagnostics::log_error("Altium import", &anyhow::anyhow!(message.clone()));
                self.document_state.error_notice = Some(ErrorNotice::import(message));
                return iced::Task::none();
            }
        };
        let task = match self.open_document_path(outcome.open.clone()) {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Open imported Altium design", &error);
                self.document_state.error_notice = Some(ErrorNotice::import(format!(
                    "Converted to {}, but it could not be opened: {error:#}",
                    outcome.open.display()
                )));
                return iced::Task::none();
            }
        };
        // Lossy steps are a review item, not an error: the files exist.
        if !outcome.warnings.is_empty() {
            self.document_state.error_notice = Some(ErrorNotice::import_advisory(format!(
                "Review before fabrication:\n{}",
                altium_import::warnings_text(&outcome.warnings)
            )));
        }
        self.refresh_panel_ctx();
        task
    }
}
