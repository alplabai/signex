//! Library lifecycle handlers — the Open / Close library commands, the
//! close-library confirmation modal (Save All / Discard All / Cancel),
//! and the left-dock library-tree node toggle.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

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
            route_open_error(&mut self.library, &path, &e);
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
        match result {
            Ok(prepared) => {
                if let Err(e) = self.library.mount_prepared(prepared) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "mount_finished: mount_prepared failed"
                    );
                    return Task::none();
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "mount_finished: prepare failed; library not mounted"
                );
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
    /// Component Preview editor against it is dirty.
    pub(super) fn handle_close_library(&mut self, path: std::path::PathBuf) -> Task<Message> {
        // If any Component Preview editors against this library
        // are dirty, divert to the confirm modal so the user
        // can Save All / Discard All / Cancel rather than
        // losing the edits silently. The modal handler
        // (`CloseLibraryConfirm`) finishes the close once the
        // user picks an option.
        let dirty = self.library.dirty_editors_for_library(&path);
        if dirty.is_empty() {
            self.library.close_library(&path);
        } else {
            let library_name = self
                .library
                .library_at(&path)
                .map(|lib| lib.display_name.clone())
                .unwrap_or_else(|| {
                    path.file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                });
            self.library.close_library_confirm = Some(CloseLibraryConfirmState {
                library_path: path,
                library_name,
                dirty_editors: dirty,
            });
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
        // Direct opener for the modal — used when callers
        // already know the dirty list (e.g. a future
        // workspace-close batch op). For the user-driven
        // close path, `CloseLibrary` is the entry point and
        // it diverts here automatically.
        let library_name = self
            .library
            .library_at(&library_path)
            .map(|lib| lib.display_name.clone())
            .unwrap_or_else(|| {
                library_path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| library_path.display().to_string())
            });
        self.library.close_library_confirm = Some(CloseLibraryConfirmState {
            library_path,
            library_name,
            dirty_editors,
        });
        Task::none()
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
                // Drop every dirty editor and proceed with the close.
                // `close_library` retains-not by `library_path`, so
                // this happens automatically as part of the close.
                self.library.close_library(&confirm.library_path);
            }
            CloseLibraryChoice::SaveAll => {
                // Persist every dirty editor's row through the
                // adapter (`handle_save_row` already runs the
                // hash + commit cycle), then close the
                // library. Failures are logged; we still
                // proceed with the close so the user isn't
                // trapped (the rows stay on disk in their
                // last good state).
                for address in &confirm.dirty_editors {
                    self.handle_save_row(address);
                }
                self.library.close_library(&confirm.library_path);
            }
        }
        Task::none()
    }

    /// Toggle the Library left-dock panel's library tree node at `idx`.
    pub(super) fn handle_toggle_library_tree_node(&mut self, idx: usize) -> Task<Message> {
        if let Some(slot) = self.library.expanded.get_mut(idx) {
            *slot = !*slot;
        }
        Task::none()
    }
}
