//! External-edit poll handlers (#531): refresh the `OpenLibrary` caches of
//! a `.snxlib` another process changed. The timer and the off-thread `stat`
//! live in `library::external_poll`.

use super::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::library::external_poll;

impl Signex {
    /// Timer tick: start the off-thread `stat` pass, unless the previous
    /// one is still running (or stuck past its timeout) or nothing is
    /// mounted.
    pub(super) fn handle_external_edits_poll_ticked(&mut self) -> Task<Message> {
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
    /// silently. A pass that timed out is reported once, not per tick.
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
            external_poll::PollOutcome::Changed(changed) => changed,
        };
        if std::mem::take(&mut self.library.external_poll_stall_reported) {
            crate::diagnostics::log_info("[library] external-edit checks resumed");
        }
        let mut refreshed = false;
        for (path, stamp) in &changed {
            let Some(lib) = self.library.library_at(path) else {
                continue;
            };
            // The caches were refilled under this exact stamp while the pass
            // ran (an in-app write): the result is stale, nothing to do.
            if lib.observed_stamp == Some(*stamp) {
                continue;
            }
            let name = lib.display_name.clone();
            let own_edit = lib.own_edit_unstamped;
            refreshed = true;
            match self.library.refresh_components(path) {
                Ok(()) if own_edit => {}
                Ok(()) => crate::diagnostics::log_info(format!(
                    "[library] {name} changed on disk; reloaded"
                )),
                Err(error) => {
                    crate::diagnostics::log_warning(format!(
                        "[library] {name} changed on disk but could not be reloaded: {error}"
                    ));
                    // Re-stamp so a persistent failure is reported once,
                    // not on every tick.
                    if let Some(lib) = self.library.library_at_mut(path) {
                        lib.observed_stamp = Some(*stamp);
                    }
                }
            }
        }
        if refreshed {
            self.refresh_panel_ctx();
        }
        Task::none()
    }
}
