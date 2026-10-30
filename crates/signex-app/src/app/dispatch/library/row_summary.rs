//! The off-thread read behind the Properties panel's library-row summaries
//! (#859). See [`crate::library::row_summaries`].

use iced::Task;

use super::*;
use crate::library::row_summaries::RowSummaries;
use crate::library::shared_resolver::OneShotCell;

impl Signex {
    /// Start the read `refresh_panel_ctx` asked for, unless its answer is
    /// already held or the read is already running.
    pub(crate) fn start_row_summary_read(&mut self) -> Option<Task<Message>> {
        let (generation, request) = self.library.row_summary_job.begin()?;
        let resolver = self.library.shared_resolver();
        let turn = self.library.writes.read_turn(request.root());
        Some(Task::perform(
            async move {
                let summaries = tokio::task::spawn_blocking(move || {
                    turn.wait();
                    request.read(&resolver)
                })
                .await
                .ok();
                OneShotCell::new(summaries)
            },
            move |readout| {
                Message::Library(LibraryMessage::RowSummariesRead {
                    generation,
                    readout,
                })
            },
        ))
    }

    /// Keep a finished read if it is the one in flight, report what failed
    /// in it, and redraw the panel with it.
    pub(super) fn handle_row_summaries_read(
        &mut self,
        generation: u64,
        readout: OneShotCell<RowSummaries>,
    ) {
        let answer = match readout.take() {
            Some(Some(summaries)) => Some(summaries),
            Some(None) => {
                crate::diagnostics::log_warning(
                    "[library] reading the selected row's symbol and footprint did not finish; \
                     the Properties panel keeps what it showed",
                );
                None
            }
            None => return,
        };
        if let (Some(summaries), Some(request)) = (
            answer.as_ref(),
            self.library.row_summary_job.request_of(generation),
        ) {
            request.report_failures(summaries);
        }
        if self.library.row_summary_job.finish(generation, answer) {
            self.refresh_panel_ctx();
        }
    }
}
