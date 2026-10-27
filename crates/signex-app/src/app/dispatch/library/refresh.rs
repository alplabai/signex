//! Completion of an off-thread library refresh (#1374).

use iced::Task;
use signex_library::LibraryError;

use super::*;
use crate::library::refresh::{RefreshFollowUp, RefreshSnapshotCell};

impl Signex {
    /// Request an off-thread refresh of the library at `root`; the caches
    /// update when the result lands (see [`Self::handle_refresh_finished`]).
    pub(crate) fn refresh_library_task(
        &mut self,
        root: &std::path::Path,
        follow_up: RefreshFollowUp,
    ) -> Task<Message> {
        crate::library::refresh::refresh_task(&mut self.library, root, follow_up)
    }

    /// Apply a finished refresh unless it is stale, then run the follow-ups
    /// that were waiting for it.
    pub(super) fn handle_refresh_finished(
        &mut self,
        path: std::path::PathBuf,
        generation: u64,
        snapshot: RefreshSnapshotCell,
    ) {
        let snapshot = snapshot.take().unwrap_or_else(|| {
            Err(LibraryError::Backend(
                "refresh result was already taken".into(),
            ))
        });
        let finish = self.library.finish_refresh(&path, generation, snapshot);
        // The library's path now: a relocation may have moved it while the
        // read ran.
        let path = finish.root;
        let failure = match finish.applied {
            Some(Err(error)) => Some(error),
            Some(Ok(())) => None,
            None => {
                tracing::debug!(
                    target: "signex::library",
                    path = %path.display(),
                    "refresh_finished: superseded or library closed; nothing applied"
                );
                None
            }
        };
        if let Some(error) = &failure {
            // One Messages warning per failed read. A follow-up with its own
            // wording (external edit, pricing) replaces the generic one.
            let has_own_wording = finish.follow_ups.iter().any(|f| {
                matches!(
                    f,
                    RefreshFollowUp::ExternalEdit { .. } | RefreshFollowUp::PricingSaved
                )
            });
            if !has_own_wording {
                let name = self.library_display_name(&path);
                crate::diagnostics::log_warning(format!(
                    "[library] refresh of {name} failed ({error}); its cache may be stale"
                ));
            }
        }
        for follow_up in finish.follow_ups {
            match &failure {
                None => self.after_refresh_applied(&path, follow_up),
                Some(error) => self.after_refresh_failed(&path, follow_up, error),
            }
        }
    }

    fn after_refresh_applied(&mut self, path: &std::path::Path, follow_up: RefreshFollowUp) {
        match follow_up {
            RefreshFollowUp::Nothing | RefreshFollowUp::PricingSaved => {}
            RefreshFollowUp::CleanEditors(rows) => self.refresh_clean_editors(path, &rows),
            RefreshFollowUp::ExternalEdit { own_edit, .. } => {
                if !own_edit {
                    let name = self.library_display_name(path);
                    crate::diagnostics::log_info(format!(
                        "[library] {name} changed on disk; reloaded"
                    ));
                }
                self.refresh_panel_ctx();
            }
        }
    }

    fn after_refresh_failed(
        &mut self,
        path: &std::path::Path,
        follow_up: RefreshFollowUp,
        error: &LibraryError,
    ) {
        let name = self.library_display_name(path);
        match follow_up {
            RefreshFollowUp::ExternalEdit { stamp, .. } => {
                crate::diagnostics::log_warning(format!(
                    "[library] {name} changed on disk but could not be reloaded: {error}"
                ));
                // Re-stamp so a persistent failure is reported once, not on
                // every tick.
                if let Some(lib) = self.library.library_at_mut(path) {
                    lib.observed_stamp = Some(stamp);
                }
                self.refresh_panel_ctx();
            }
            RefreshFollowUp::PricingSaved => {
                crate::diagnostics::log_warning(format!(
                    "{name}: prices were saved but the library view could not be reloaded: {error}"
                ));
            }
            // Reported once by `handle_refresh_finished`.
            RefreshFollowUp::Nothing | RefreshFollowUp::CleanEditors(_) => {}
        }
    }

    fn library_display_name(&self, path: &std::path::Path) -> String {
        self.library
            .library_at(path)
            .map(|lib| lib.display_name.clone())
            .unwrap_or_else(|| path.display().to_string())
    }
}

impl Signex {
    /// Land the refresh `refresh_library_task` last started for `path`, as
    /// the runtime would once its blocking read finished: same read, same
    /// completion handler, no thread. For tests, which cannot run a `Task`.
    #[doc(hidden)]
    pub fn land_pending_refresh(&mut self, path: &std::path::Path) {
        let generation = self.library.refresh_generations[path];
        self.land_refresh_with_generation(path, generation);
    }

    /// [`Self::land_pending_refresh`] for a read stamped `generation`,
    /// which the library may have moved past.
    #[doc(hidden)]
    pub fn land_refresh_with_generation(&mut self, path: &std::path::Path, generation: u64) {
        let library_id = self
            .library
            .library_at(path)
            .expect("library mounted")
            .library_id;
        let adapter = self.library.set.get(library_id).expect("adapter mounted");
        let snapshot = crate::library::state::LibrarySnapshot::read(adapter, path);
        let cell = RefreshSnapshotCell::new(snapshot);
        self.handle_refresh_finished(path.to_path_buf(), generation, cell);
    }
}
