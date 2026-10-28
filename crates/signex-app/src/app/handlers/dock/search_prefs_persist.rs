//! Debounced, off-thread persistence of the search boxes (#859).
//!
//! The decisions live in `crate::persist::SearchPrefsSync`; this file only
//! turns them into `Task`s and reports failures to the Messages panel,
//! mirroring `layout_persist.rs` for the dock layout.

use iced::Task;

use crate::app::{Message, Signex};
use crate::persist::{SEARCH_PREFS_SAVE_DEBOUNCE, SearchPrefsMsg, SearchPrefsWrite};

impl Signex {
    /// A Library Browser search box changed: record it and arm the debounce.
    pub(crate) fn persist_library_search_soon(
        &mut self,
        library: &std::path::Path,
        query: &str,
    ) -> Task<Message> {
        let armed = self
            .document_state
            .search_prefs
            .set_library_search(library, query);
        search_prefs_timer(armed)
    }

    pub(crate) fn dispatch_search_prefs_message(&mut self, msg: SearchPrefsMsg) -> Task<Message> {
        match msg {
            SearchPrefsMsg::SaveDebounceElapsed(generation) => {
                match self.document_state.search_prefs.take_if_current(generation) {
                    Some(write) => write_search_prefs_task(write),
                    None => Task::none(),
                }
            }
            SearchPrefsMsg::Saved(Ok(())) => Task::none(),
            SearchPrefsMsg::Saved(Err(reason)) => {
                self.document_state.search_prefs.write_failed();
                crate::diagnostics::log_error(
                    "Search box contents were not saved; retrying on the next edit or at quit",
                    &anyhow::anyhow!(reason),
                );
                self.sync_diagnostics_panel_ctx();
                Task::none()
            }
        }
    }

    /// Quit path: flush every debounced preference write (dock layout, then
    /// search boxes, then in-flight document saves, then in-flight autosave
    /// writes), wait for the ones in flight, then run `next`.
    pub(crate) fn flush_quit_writes_then(&mut self, next: Task<Message>) -> Task<Message> {
        // A pricing walk must not write into a library after quit, and an
        // in-flight save + commit is waited for.
        let next = self.wait_for_pricing_then(next);
        // Every `background_pref_write` started so far (Save Preferences,
        // recent projects, filter presets, toggles) must land before exit
        // (#1351).
        let next = wait_for_pref_writes_then(next);
        // Profile edits still queued (the machine's rules, a profile edited in
        // Preferences) are written before the process goes.
        let next = self.wait_for_profile_writes_then(next);
        let search = self.flush_search_prefs_then(next);
        // After the document saves: a save supersedes its recovery copy, and
        // the removal that follows must finish before the process exits.
        let autosave = self.wait_for_autosave_then(search);
        let documents = self.wait_for_document_saves_then(autosave);
        self.flush_dock_layout_then(documents)
    }

    /// Write any pending search prefs now and wait for a write that is
    /// already running, then run `next` (the quit path).
    fn flush_search_prefs_then(&mut self, next: Task<Message>) -> Task<Message> {
        let prefs = &mut self.document_state.search_prefs;
        let barrier = prefs.barrier();
        let pending = prefs.flush().map(write_search_prefs_task);
        let drained = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || barrier.wait())
                    .await
                    .map_err(|join_error| {
                        format!("waiting for the search prefs write failed: {join_error}")
                    })
            },
            |result| Message::SearchPrefs(SearchPrefsMsg::Saved(result)),
        );
        match pending {
            Some(write) => write.chain(drained).chain(next),
            None => drained.chain(next),
        }
    }
}

/// Wait (bounded) for every in-flight ordered preference write, then `next`.
fn wait_for_pref_writes_then(next: Task<Message>) -> Task<Message> {
    #[cfg(test)]
    let probe = crate::fonts::take_wait_probe();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                #[cfg(test)]
                if let Some(probe) = &probe {
                    probe.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if !crate::fonts::wait_for_pref_writes(PREF_WRITE_QUIT_LIMIT) {
                    crate::diagnostics::log_error(
                        "Preferences were not fully saved before exit",
                        &anyhow::anyhow!(
                            "a preference write was still running after {} s",
                            PREF_WRITE_QUIT_LIMIT.as_secs()
                        ),
                    );
                }
            })
            .await
            .ok();
        },
        |()| Message::noop(),
    )
    .chain(next)
}

/// How long quit waits for preference writes before giving up.
const PREF_WRITE_QUIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

fn search_prefs_timer(armed: Option<u64>) -> Task<Message> {
    let Some(generation) = armed else {
        return Task::none();
    };
    // The `async` block keeps the timer lazy: `tokio::time::sleep`
    // registers with the reactor when called, and `update` runs outside
    // the runtime's context.
    Task::perform(
        async { tokio::time::sleep(SEARCH_PREFS_SAVE_DEBOUNCE).await },
        move |()| Message::SearchPrefs(SearchPrefsMsg::SaveDebounceElapsed(generation)),
    )
}

fn write_search_prefs_task(write: SearchPrefsWrite) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || write.run())
                .await
                .map_err(|join_error| format!("the write task failed: {join_error}"))
                .and_then(|result| result)
        },
        |result| Message::SearchPrefs(SearchPrefsMsg::Saved(result)),
    )
}
