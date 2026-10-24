//! Refresh All Pricing: start the background walk, refuse a second one, and
//! report the outcome in the Messages panel.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use iced::Task;

use super::*;
use crate::library::pricing::{LineLevel, PricingRefreshRun, report_lines, run_job};

impl Signex {
    /// Menu entry point. `update` only validates and spawns; the network and
    /// the library write lock stay on the blocking thread.
    pub(crate) fn start_pricing_refresh(
        &mut self,
        row: crate::app::LibraryRowRef,
    ) -> Task<Message> {
        let Some(library) = self.library.library_at(&row.root) else {
            crate::diagnostics::log_warning(format!(
                "Refresh All Pricing: {} is not open; open it first.",
                row.root.display()
            ));
            return Task::none();
        };
        let name = library.display_name.clone();
        if let Some(running) = &self.library.pricing_refresh {
            let which = running
                .library_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| running.library_path.display().to_string());
            crate::diagnostics::log_warning(format!(
                "Refresh All Pricing refused for {name}: a pricing refresh of {which} is still running."
            ));
            return Task::none();
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.library.pricing_refresh = Some(PricingRefreshRun {
            library_path: row.root.clone(),
            cancel: Arc::clone(&cancel),
        });
        crate::diagnostics::log_info(format!("Refresh All Pricing started for {name}."));
        let enabled = self.library.settings.preferred_order.clone();
        let path = row.root;
        let reply_path = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || run_job(&path, &enabled, &cancel))
                    .await
                    .unwrap_or_else(|e| Err(format!("pricing worker panicked: {e}")))
            },
            move |outcome| {
                Message::Library(LibraryMessage::PricingRefreshFinished {
                    library_path: reply_path.clone(),
                    outcome: Arc::new(outcome),
                })
            },
        )
    }

    /// The walk is over: free the slot, report, and re-read the library so
    /// the new prices show.
    pub(super) fn handle_pricing_refresh_finished(
        &mut self,
        library_path: PathBuf,
        outcome: Arc<Result<signex_library::PricingRefreshReport, String>>,
    ) -> Task<Message> {
        self.library.pricing_refresh = None;
        let name = self
            .library
            .library_at(&library_path)
            .map(|l| l.display_name.clone())
            .unwrap_or_else(|| library_path.display().to_string());
        match outcome.as_ref() {
            Ok(report) => {
                for (level, line) in report_lines(&name, report) {
                    match level {
                        LineLevel::Info => crate::diagnostics::log_info(line),
                        LineLevel::Warning => crate::diagnostics::log_warning(line),
                    }
                }
            }
            Err(reason) => crate::diagnostics::log_warning(format!(
                "Refresh All Pricing failed for {name}: {reason}"
            )),
        }
        if self.library.library_at(&library_path).is_some()
            && let Err(e) = self.library.refresh_components(&library_path)
        {
            crate::diagnostics::log_warning(format!(
                "{name}: prices were saved but the library view could not be reloaded: {e}"
            ));
        }
        Task::none()
    }
}
