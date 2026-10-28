//! External-edit poll handlers (#531): refresh the `OpenLibrary` caches of
//! a `.snxlib` another process changed. The timer and the off-thread `stat`
//! live in `library::external_poll`.

use super::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::library::external_poll;
use crate::library::refresh::RefreshFollowUp;

impl Signex {
    /// Timer tick: start the off-thread `stat` pass, unless the previous
    /// one is still running (or stuck past its timeout) or nothing is
    /// mounted.
    pub(super) fn handle_external_edits_poll_ticked(&mut self) -> Task<Message> {
        // The only clock `view` gets (ages of cached prices).
        self.library.clock = chrono::Utc::now();
        // The same tick re-checks the project tree's `(missing)` marks (#111).
        let stat_pass = self.start_library_stat_pass();
        match self.start_project_fs_probe(true) {
            Some(probe) => Task::batch([stat_pass, probe]),
            None => stat_pass,
        }
    }

    fn start_library_stat_pass(&mut self) -> Task<Message> {
        let stat_running = &self.library.external_poll_stat_running;
        if self.library.external_poll_in_flight || stat_running.load(Ordering::SeqCst) {
            return Task::none();
        }
        let watched = self.library.watched_libraries();
        if watched.is_empty() {
            return Task::none();
        }
        self.library.external_poll_in_flight = true;
        let stat_running = Arc::clone(stat_running);
        Task::perform(
            external_poll::changed_libraries(watched, stat_running),
            |outcome| Message::Library(LibraryMessage::ExternalEditsPolled(outcome)),
        )
    }

    /// `stat` pass done: refresh every library it named and tell the user
    /// in the Messages panel, so an external edit never changes the view
    /// silently. A pass that timed out or failed is reported once, not per
    /// tick.
    pub(super) fn handle_external_edits_polled(
        &mut self,
        outcome: external_poll::PollOutcome,
    ) -> Task<Message> {
        self.library.external_poll_in_flight = false;
        let changed = match outcome {
            external_poll::PollOutcome::TimedOut => {
                if !self.library.external_poll_stall_reported {
                    self.library.external_poll_stall_reported = true;
                    crate::diagnostics::log_warning(format!(
                        "[library] checking mounted libraries for external edits took longer \
                         than {} s (an unreachable network drive?); paused until it responds",
                        external_poll::POLL_TIMEOUT.as_secs()
                    ));
                }
                return Task::none();
            }
            external_poll::PollOutcome::Failed(reason) => {
                if !self.library.external_poll_failure_reported {
                    self.library.external_poll_failure_reported = true;
                    crate::diagnostics::log_warning(format!(
                        "[library] checking mounted libraries for external edits failed: \
                         {reason}; retrying quietly until a check succeeds"
                    ));
                }
                return Task::none();
            }
            external_poll::PollOutcome::Changed(changed) => changed,
        };
        let stalled = std::mem::take(&mut self.library.external_poll_stall_reported);
        let failed = std::mem::take(&mut self.library.external_poll_failure_reported);
        if stalled || failed {
            crate::diagnostics::log_info("[library] external-edit checks resumed");
        }
        let mut tasks = Vec::new();
        for (path, stamp) in &changed {
            let Some(lib) = self.library.library_at(path) else {
                continue;
            };
            // A refresh of this library is already on its way: its caches
            // are about to change, and what the stamp shows may be the
            // app's own write.
            if self.library.refresh_in_flight(path) {
                continue;
            }
            // The caches were refilled under this exact stamp while the pass
            // ran (an in-app write): the result is stale, nothing to do.
            if lib.observed_stamp == Some(*stamp) {
                continue;
            }
            // Quiet only when the file is exactly what the app's own class
            // write left; any later external edit changes the stamp and is
            // reported as usual.
            let own_edit = lib.own_edit_stamp == Some(*stamp);
            tasks.push(self.refresh_library_task(
                path,
                RefreshFollowUp::ExternalEdit {
                    stamp: *stamp,
                    own_edit,
                },
            ));
        }
        Task::batch(tasks)
    }
}
