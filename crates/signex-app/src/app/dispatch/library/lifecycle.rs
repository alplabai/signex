//! Library lifecycle handlers — the Open / Close library commands, the
//! close-library confirmation modal (Save All / Discard All / Cancel),
//! and the left-dock library-tree node toggle.
//!
//! Split out of the library dispatcher (`dispatch/library`).

use super::*;

impl Signex {
    /// File ▸ Library ▸ Open Library… — runs `rfd::AsyncFileDialog` on
    /// the directory level and lands in [`LibraryMessage::OpenLibraryAt`].
    pub(super) fn handle_open_library_dialog(&mut self) -> Task<Message> {
        Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Open Library (*.snxlib/)")
                    .pick_folder()
                    .await
                    .map(|f| f.path().to_path_buf())
            },
            |path| Message::Library(LibraryMessage::OpenLibraryAt(path)),
        )
    }

    /// Result of the `rfd` directory pick — mount the library at `path`,
    /// routing any open error into the recovery flow.
    pub(super) fn handle_open_library_at(&mut self, path: std::path::PathBuf) -> Task<Message> {
        if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
            tracing::warn!(target: "signex::library", error = %e, path = %path.display(), "open_library failed");
            // Recoverable failures open their recovery dialog; anything
            // else must still reach the user, so raise the error card.
            if !route_open_error(&mut self.library, &path, &e) {
                self.document_state.error_notice =
                    Some(open_error_notice(&self.library, &path, "open library", &e));
            }
        }
        Task::none()
    }

    /// A `.snxlib` mount prepared off the UI thread has landed — issue
    /// #99 part 2c.
    ///
    /// Order matters and both early returns are load-bearing:
    ///
    /// 1. **Intent first, from the map.** `take_mount_intent` returning
    ///    `None` means `close_library` cancelled this request while the
    ///    preparation was in flight, so the payload is dropped rather
    ///    than re-mounting a library the user just closed. Reading the
    ///    intent from the map — not from a value captured at spawn time —
    ///    is what preserves a `Silent` → `OpenBrowserTab` upgrade made by
    ///    a second request.
    /// 2. **Then the payload.** The cell is one-shot; a second read
    ///    yields `None`. iced does not clone a `Task::perform` result
    ///    today, so that should be unreachable — it warns instead of
    ///    panicking because a dropped mount is a stale cache, not a
    ///    corrupt one.
    ///
    /// A prepare error only warns, which is exactly what both converted
    /// call sites did synchronously. The `rfd` directory-pick path
    /// (`handle_open_library_at`) is deliberately still synchronous and
    /// keeps routing its errors into the recovery flow.
    pub(super) fn handle_mount_finished(
        &mut self,
        path: std::path::PathBuf,
        prepared: crate::library::mount::PreparedMountCell,
    ) -> Task<Message> {
        let Some(intent) = self.library.take_mount_intent(&path) else {
            tracing::debug!(
                target: "signex::library",
                path = %path.display(),
                "mount_finished: no longer pending (closed while preparing); discarding"
            );
            return Task::none();
        };
        let Some(result) = prepared.take() else {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                "mount_finished: payload already taken; library not mounted"
            );
            return Task::none();
        };
        // Raised only for a mount the user asked for by name (#532).
        // `MountIntent::Silent` is the background auto-mount of a
        // project's libraries — popping a blocking card there would
        // greet the user with one modal per stale path at startup, for
        // an operation they never invoked.
        let mut report = |path: &std::path::Path, reason: &str| {
            let error = anyhow::anyhow!("could not open library {}: {reason}", path.display());
            crate::diagnostics::log_error("Failed to mount library", &error);
            if matches!(intent, crate::library::mount::MountIntent::OpenBrowserTab) {
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::open(format!("{error:#}")));
            }
        };
        match result {
            Ok(prepared) => {
                let library_id = prepared.entry.library_id;
                let newly_mounted = self.library.library_at(&prepared.entry.root).is_none();
                if let Err(e) = self.library.mount_prepared(prepared) {
                    report(&path, &e.to_string());
                    return Task::none();
                }
                self.library.drift.check_pending = true;
                // Sheets opened before this library mounted skipped its
                // symbols; scan them now (#1449).
                if newly_mounted {
                    self.rescan_library_updates_after_mount(library_id);
                }
            }
            Err(e) => {
                report(&path, &e);
                return Task::none();
            }
        }
        match intent {
            crate::library::mount::MountIntent::Silent => Task::none(),
            crate::library::mount::MountIntent::OpenBrowserTab => {
                self.finish_open_library_browser(path)
            }
        }
    }

    /// Close an open library — diverts to the confirm modal when any
    /// Component Preview editor against it is dirty, so the user can Save
    /// All / Discard All / Cancel rather than lose the edits silently.
    pub(crate) fn handle_close_library(&mut self, path: std::path::PathBuf) -> Task<Message> {
        let dirty = self.library.dirty_editors_for_library(&path);
        if dirty.is_empty() {
            self.close_library_or_defer(&path);
        } else {
            self.raise_close_library_prompt(path, dirty);
        }
        Task::none()
    }

    /// Direct opener for the close-library confirm modal — used when
    /// callers already know the dirty list.
    pub(super) fn handle_confirm_close_library(
        &mut self,
        library_path: std::path::PathBuf,
        dirty_editors: Vec<EditorAddress>,
    ) -> Task<Message> {
        self.raise_close_library_prompt(library_path, dirty_editors);
        Task::none()
    }

    /// Open the close-library confirm modal for `dirty_editors`.
    pub(super) fn raise_close_library_prompt(
        &mut self,
        library_path: std::path::PathBuf,
        dirty_editors: Vec<EditorAddress>,
    ) {
        let library_name = self.library.library_label(&library_path);
        self.library.close_library_confirm = Some(CloseLibraryConfirmState {
            library_path,
            library_name,
            dirty_editors,
        });
    }

    /// User picked Save All / Discard All / Cancel in the close prompt.
    pub(super) fn handle_close_library_confirm(
        &mut self,
        choice: CloseLibraryChoice,
    ) -> Task<Message> {
        let Some(confirm) = self.library.close_library_confirm.take() else {
            return Task::none();
        };
        match choice {
            CloseLibraryChoice::Cancel => {
                // No state change — user kept the library open.
            }
            CloseLibraryChoice::DiscardAll => {
                // Drop the library's editors now: a close that waits for
                // writes must not find the discarded drafts dirty and ask
                // again when it comes due.
                let root = &confirm.library_path;
                self.library
                    .editors
                    .retain(|key, _| key.library_path != *root);
                self.close_library_or_defer(root);
            }
            CloseLibraryChoice::SaveAll => {
                // Persist every dirty editor's row through the adapter
                // (staged PDFs are written on a worker first), then close
                // the library. A failed save keeps the library open with
                // its drafts and raises the Save Failed card: closing
                // anyway would turn "Save All" into a silent discard
                // (#110). The user can retry, or pick Discard All.
                return self.begin_save_all(
                    "Close Library",
                    &[],
                    &confirm.dirty_editors,
                    crate::app::contracts::SaveAllNext::CloseLibrary {
                        library_path: confirm.library_path,
                    },
                );
            }
        }
        Task::none()
    }
}
