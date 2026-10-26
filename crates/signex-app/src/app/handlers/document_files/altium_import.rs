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
        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;
        if let Some(running) = &self.document_state.altium_import_running {
            let message = format!(
                "An Altium import of {} is still running; wait for it to finish \
                 or cancel it from the status bar.",
                running.input.display()
            );
            crate::diagnostics::log_warning(&message);
            self.document_state.error_notice = Some(ErrorNotice::import(message));
            return iced::Task::none();
        }
        let binary = match altium_import::locate() {
            Ok(binary) => binary,
            Err(message) => {
                crate::diagnostics::log_error("Altium import", &anyhow::anyhow!(message.clone()));
                self.document_state.error_notice = Some(ErrorNotice::import(message));
                return iced::Task::none();
            }
        };
        let out_dir = altium_import::output_dir_for(&input);
        log::info!(
            "Altium import: converting {} into {}",
            input.display(),
            out_dir.display()
        );
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.document_state.altium_import_running = Some(crate::app::state::AltiumImportRun {
            input: input.clone(),
            cancel: cancel.clone(),
        });
        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    altium_import::run(&binary, &input, &out_dir, altium_import::TIMEOUT, &cancel)
                })
                .await
                .unwrap_or_else(|error| Err(format!("Altium import did not complete: {error}")))
            },
            |result| Message::File(FileMsg::AltiumImportFinished(result)),
        )
    }

    /// Status-bar Cancel: ask the running converter to stop. Its result
    /// still arrives through [`Self::handle_altium_import_finished`].
    pub(crate) fn handle_altium_import_cancel(&mut self) {
        if let Some(run) = &self.document_state.altium_import_running {
            run.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            log::info!("Altium import: cancelling {}", run.input.display());
        }
    }

    pub(crate) fn handle_altium_import_finished(
        &mut self,
        result: Result<ImportOutcome, String>,
    ) -> iced::Task<Message> {
        self.document_state.altium_import_running = None;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(message) => {
                crate::diagnostics::log_error("Altium import", &anyhow::anyhow!(message.clone()));
                self.document_state.error_notice = Some(ErrorNotice::import(message));
                return iced::Task::none();
            }
        };
        // Lossy steps are the import's most important output: the Messages
        // panel keeps them after the card is dismissed or replaced.
        for (message, count) in &outcome.warnings {
            crate::diagnostics::log_warning(format!(
                "Altium import: {message} (×{count}); review before fabrication"
            ));
        }
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
                "Review before fabrication (also in the Messages panel and {}):\n{}",
                altium_import::REPORT_FILE,
                altium_import::warnings_text(&outcome.warnings)
            )));
        }
        self.refresh_panel_ctx();
        task
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_of(input: &str) -> crate::app::state::AltiumImportRun {
        crate::app::state::AltiumImportRun {
            input: PathBuf::from(input),
            cancel: Default::default(),
        }
    }

    fn logged(needle: &str) -> usize {
        let _ = crate::diagnostics::init_logging();
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|e| e.message.contains(needle))
            .count()
    }

    #[test]
    fn a_failed_import_shows_the_card_and_reaches_messages() {
        let reason = "altium-1097-failure-unique";
        let before = logged(reason);
        let (mut app, _boot) = Signex::new();
        app.document_state.altium_import_running = Some(run_of("/d/B.PrjPcb"));

        let _ = app.update(Message::File(FileMsg::AltiumImportFinished(Err(
            reason.to_string()
        ))));

        let card = app.document_state.error_notice.as_ref().expect("card");
        assert!(card.detail.contains(reason));
        assert_eq!(logged(reason), before + 1);
        assert!(app.document_state.altium_import_running.is_none());
    }

    #[test]
    fn import_warnings_reach_messages_and_the_document_opens() {
        let warning = "altium-1097-arc-flattened-unique";
        let before = logged(warning);
        let (mut app, _boot) = Signex::new();
        let open = PathBuf::from("/d/B-signex/A.snxsch");

        let _ = app.update(Message::File(FileMsg::AltiumImportFinished(Ok(
            ImportOutcome {
                open: open.clone(),
                warnings: vec![(warning.to_string(), 3)],
            },
        ))));

        assert_eq!(logged(warning), before + 1, "every warning is logged");
        let card = app.document_state.error_notice.as_ref().expect("advisory");
        assert!(card.detail.contains(warning));
        assert!(
            app.document_state.pending_opens.contains(&open),
            "the converted sheet is being opened"
        );
    }

    #[test]
    fn a_second_import_is_refused_while_one_runs() {
        let (mut app, _boot) = Signex::new();
        let running = run_of("/d/altium-1097-running-unique.PrjPcb");
        app.document_state.altium_import_running = Some(running.clone());
        let before = logged("altium-1097-running-unique");

        let _ = app.update(Message::File(FileMsg::AltiumImportPicked(Some(
            PathBuf::from("/d/Other.PrjPcb"),
        ))));

        assert_eq!(logged("altium-1097-running-unique"), before + 1);
        assert_eq!(app.document_state.altium_import_running, Some(running));
    }

    #[test]
    fn cancel_sets_the_running_imports_flag() {
        let (mut app, _boot) = Signex::new();
        let running = run_of("/d/B.PrjPcb");
        app.document_state.altium_import_running = Some(running.clone());

        let _ = app.update(Message::Ui(crate::app::UiMsg::StatusBar(
            crate::app::StatusBarRequest::CancelAltiumImport,
        )));

        assert!(running.cancel.load(std::sync::atomic::Ordering::Relaxed));
        assert!(
            app.document_state.altium_import_running.is_some(),
            "the run stays tracked until its result arrives"
        );
    }
}
