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
    ) -> Task<Message> {
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
                    RefreshFollowUp::ExternalEdit { .. }
                        | RefreshFollowUp::PricingSaved
                        | RefreshFollowUp::HistoryRestored
                )
            });
            if !has_own_wording {
                let name = self.library_display_name(&path);
                crate::diagnostics::log_warning(format!(
                    "[library] refresh of {name} failed ({error}); its cache may be stale"
                ));
            }
        }
        let mut tasks = Vec::new();
        for follow_up in finish.follow_ups {
            match &failure {
                None => tasks.push(self.after_refresh_applied(&path, follow_up)),
                Some(error) => self.after_refresh_failed(&path, follow_up, error),
            }
        }
        Task::batch(tasks)
    }

    /// Apply the previews a worker re-resolved after a primitive save,
    /// unless a newer plan or a closed library made them stale.
    pub(super) fn handle_previews_reread(
        &mut self,
        readout: crate::library::preview_refresh::PreviewReadoutCell,
    ) {
        match readout.take() {
            Some(Some(readout)) => self.library.apply_preview_readout(readout),
            Some(None) => crate::diagnostics::log_warning(
                "[library] re-reading previews after a primitive save did not finish; \
                 open previews may show the earlier copy",
            ),
            None => {}
        }
    }

    /// Apply a row re-read by a worker, unless it went stale.
    pub(super) fn handle_editor_reloaded(
        &mut self,
        readout: crate::library::shared_resolver::OneShotCell<
            crate::library::preview_reads::EditorReloadOutcome,
        >,
    ) -> Task<Message> {
        use crate::library::preview_reads::ReloadApplied;
        let outcome = match readout.take() {
            Some(Some(outcome)) => outcome,
            Some(None) => {
                crate::diagnostics::log_warning(
                    "[library] re-reading a component row did not finish; \
                     the open copy was left as it is",
                );
                return Task::none();
            }
            None => return Task::none(),
        };
        let address = outcome.address().clone();
        match self.library.apply_editor_reload(outcome) {
            ReloadApplied::Failed | ReloadApplied::Refused => {
                self.sync_diagnostics_panel_ctx();
                Task::none()
            }
            ReloadApplied::Replaced {
                resolve_tab: Some(tab),
            } => crate::library::preview_reads::tab_resolve_task(&mut self.library, &address, tab),
            ReloadApplied::Replaced { resolve_tab: None } | ReloadApplied::Dropped => Task::none(),
        }
    }

    /// Apply the primitives a worker looked up for a preview tab.
    pub(super) fn handle_preview_tab_resolved(
        &mut self,
        readout: crate::library::shared_resolver::OneShotCell<
            crate::library::preview_reads::TabResolveOutcome,
        >,
    ) {
        match readout.take() {
            Some(Some(outcome)) => self.library.apply_tab_resolve(outcome),
            Some(None) => crate::diagnostics::log_warning(
                "[library] looking up a component preview's primitives did not finish; \
                 switch tabs to try again",
            ),
            None => {}
        }
    }

    fn after_refresh_applied(
        &mut self,
        path: &std::path::Path,
        follow_up: RefreshFollowUp,
    ) -> Task<Message> {
        match follow_up {
            RefreshFollowUp::Nothing | RefreshFollowUp::PricingSaved => {}
            RefreshFollowUp::RevealSelected => return self.reveal_selected_browser_row(path),
            RefreshFollowUp::CleanEditors(rows) => return self.refresh_clean_editors(path, &rows),
            RefreshFollowUp::HistoryRestored => {
                crate::diagnostics::log_info(format!(
                    "[reload] LibraryBrowser tab {} refreshed",
                    path.display()
                ));
                self.refresh_panel_ctx();
            }
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
        Task::none()
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
            RefreshFollowUp::HistoryRestored => {
                let reason = error.to_string();
                let line = match reason.split_once(crate::library::refresh::REOPEN_FAILED) {
                    Some((_, open_error)) => format!(
                        "[reload] LocalGitAdapter::open({}) failed: {open_error}",
                        path.display()
                    ),
                    None => format!("[reload] reload_tables({}) failed: {error}", path.display()),
                };
                crate::diagnostics::log_warning(line);
            }
            RefreshFollowUp::PricingSaved => {
                crate::diagnostics::log_warning(format!(
                    "{name}: prices were saved but the library view could not be reloaded: {error}"
                ));
            }
            // Reported once by `handle_refresh_finished`.
            RefreshFollowUp::Nothing
            | RefreshFollowUp::RevealSelected
            | RefreshFollowUp::CleanEditors(_) => {}
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
    pub fn land_pending_refresh(&mut self, path: &std::path::Path) -> Task<Message> {
        let generation = self.library.refresh_generations[path];
        self.land_refresh_with_generation(path, generation)
    }

    /// [`Self::land_pending_refresh`] for a read stamped `generation`,
    /// which the library may have moved past.
    #[doc(hidden)]
    pub fn land_refresh_with_generation(
        &mut self,
        path: &std::path::Path,
        generation: u64,
    ) -> Task<Message> {
        let library_id = self
            .library
            .library_at(path)
            .expect("library mounted")
            .library_id;
        let adapter = self.library.set.get(library_id).expect("adapter mounted");
        let snapshot = crate::library::state::LibrarySnapshot::read(adapter, path);
        let cell = RefreshSnapshotCell::new(snapshot);
        self.handle_refresh_finished(path.to_path_buf(), generation, cell)
    }
}
