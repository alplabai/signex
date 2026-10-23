//! External-edit poll handlers (#531): refresh the `OpenLibrary` caches of
//! a `.snxlib` another process changed. The timer and the off-thread `stat`
//! live in `library::external_poll`.

use super::*;
use crate::library::external_poll;

impl Signex {
    /// Timer tick: start the off-thread `stat` pass, unless the previous
    /// one is still running or nothing is mounted.
    pub(super) fn handle_external_edits_poll_ticked(&mut self) -> Task<Message> {
        if self.library.external_poll_in_flight {
            return Task::none();
        }
        let watched = self.library.watched_libraries();
        if watched.is_empty() {
            return Task::none();
        }
        self.library.external_poll_in_flight = true;
        Task::perform(external_poll::changed_libraries(watched), |changed| {
            Message::Library(LibraryMessage::ExternalEditsPolled(changed))
        })
    }

    /// `stat` pass done: refresh every library it named and tell the user
    /// in the Messages panel, so an external edit never changes the view
    /// silently.
    pub(super) fn handle_external_edits_polled(
        &mut self,
        changed: Vec<std::path::PathBuf>,
    ) -> Task<Message> {
        self.library.external_poll_in_flight = false;
        if changed.is_empty() {
            return Task::none();
        }
        for path in &changed {
            let name = self
                .library
                .library_at(path)
                .map(|lib| lib.display_name.clone())
                .unwrap_or_else(|| path.display().to_string());
            match self.library.refresh_components(path) {
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
                        lib.observed_stamp = signex_library::DiskStamp::of(path).ok();
                    }
                }
            }
        }
        self.refresh_panel_ctx();
        Task::none()
    }
}
