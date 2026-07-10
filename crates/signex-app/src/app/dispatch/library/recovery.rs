//! Open-error recovery plumbing — the shared atomic-write helper,
//! `route_open_error` classification, and the per-choice recovery
//! actions (library-missing / git-missing / broken-binding).
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

/// Atomic write — write `bytes` to `<path>.tmp` then `rename` over
/// `path`. A crash mid-write leaves either the original file intact
/// Re-export of the shared atomic-write helper (HI-6). Lives in
/// `signex-types::atomic_io` so engine, library, and app share one
/// implementation; the function used to be a private duplicate here.
pub(super) fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    signex_types::atomic_io::atomic_write(path, bytes)
}

// ─────────────────────────────────────────────────────────────────────
// Stage 10 — recovery dialog plumbing
// ─────────────────────────────────────────────────────────────────────
//
// `LocalGitAdapter::open` returns a few recoverable error shapes that
// shouldn't drop on the floor as a bare `tracing::warn!`. The user
// either wants to point Signex at a moved file, accept that history
// is gone, or remove the library from the project entirely. The
// recovery module owns the modal layer; this section owns the
// classification + per-choice action.

use crate::library::recovery::{
    BrokenBindingChoice, GitMissingChoice, LibraryMissingChoice, RecoveryDialog,
};
use crate::library::state::LibraryState;
use signex_library::{LibraryError, LocalGitAdapter};

/// Classify a `LocalGitAdapter::open` error and, if recoverable,
/// stash the matching `RecoveryDialog` on `LibraryState::recovery`.
/// Unrecoverable errors are left alone — the caller's `tracing::warn!`
/// is the only surface.
///
/// String-matches the error message produced by `LocalGitAdapter::open`
/// because the underlying `LibraryError` enum doesn't carry structured
/// "missing-snxlib" / "missing-git" variants in v0.9. This is the
/// lower-effort path called out in `v0.9-snxlib-as-file-plan.md` §2
/// Stage H — adding `LibraryError::MissingGitRepo` /
/// `LibraryError::MissingSnxlibFile` variants is a clean follow-up
/// once the rest of v0.9 settles.
pub(crate) fn route_open_error(
    state: &mut LibraryState,
    path: &std::path::Path,
    err: &LibraryError,
) {
    // Don't clobber an already-open recovery dialog; the user resolves
    // them sequentially.
    if state.recovery.is_some() {
        return;
    }
    let dialog = match err {
        LibraryError::NotFound(msg) if msg.contains("no .snxlib") => {
            Some(RecoveryDialog::LibraryMissing {
                path: path.to_path_buf(),
            })
        }
        LibraryError::Backend(msg) if msg.starts_with("git open") => {
            // v0.9: no remote field on the manifest yet. Stage 13+ will
            // populate this from `[users.<remote>]` so the
            // "Restore from remote" button activates.
            Some(RecoveryDialog::GitMissing {
                path: path.to_path_buf(),
                remote: None,
            })
        }
        _ => None,
    };
    if let Some(d) = dialog {
        state.recovery = Some(d);
    }
}

/// Handle the user's choice from the *Library missing* recovery dialog.
pub(super) fn handle_recovery_library_missing(
    app: &mut Signex,
    choice: LibraryMissingChoice,
) -> Task<Message> {
    match choice {
        LibraryMissingChoice::Cancel => {
            app.library.recovery = None;
            Task::none()
        }
        LibraryMissingChoice::Locate => Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Locate Library (*.snxlib)")
                    .add_filter("Signex Library", &["snxlib"])
                    .pick_file()
                    .await
                    .map(|f| f.path().to_path_buf())
            },
            |path| Message::Library(LibraryMessage::RecoveryLibraryMissingLocateResult(path)),
        ),
        LibraryMissingChoice::RemoveFromProject => {
            let missing = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::LibraryMissing { path }) => path.clone(),
                _ => {
                    app.library.recovery = None;
                    return Task::none();
                }
            };
            for project in app.document_state.projects.iter_mut() {
                // Compute resolved paths up-front so the closure can
                // borrow only the indices vector, not project.data
                // (which retain's closure also tries to read).
                let resolved: Vec<std::path::PathBuf> = project
                    .data
                    .libraries
                    .iter()
                    .map(|e| project.data.resolve_library_path(e))
                    .collect();
                let mut idx = 0usize;
                project.data.libraries.retain(|_| {
                    let keep = resolved[idx] != missing;
                    idx += 1;
                    keep
                });
            }
            app.library.recovery = None;
            Task::none()
        }
    }
}

/// Handle the user's choice from the *Git missing* recovery dialog.
pub(super) fn handle_recovery_git_missing(app: &mut Signex, choice: GitMissingChoice) -> Task<Message> {
    match choice {
        GitMissingChoice::Cancel | GitMissingChoice::Skip => {
            app.library.recovery = None;
            Task::none()
        }
        GitMissingChoice::ReInit => {
            let path = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::GitMissing { path, .. }) => path.clone(),
                _ => {
                    app.library.recovery = None;
                    return Task::none();
                }
            };
            app.library.recovery = None;
            match LocalGitAdapter::recover_init(&path) {
                Ok(_) => Task::done(Message::Library(LibraryMessage::OpenLibraryAt(Some(path)))),
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "git recover-init failed"
                    );
                    Task::none()
                }
            }
        }
        GitMissingChoice::RestoreFromRemote => {
            // v0.9 leaves this disabled — the manifest doesn't carry a
            // remote yet. Treat as Cancel.
            app.library.recovery = None;
            Task::none()
        }
    }
}

/// Handle the user's choice from the *Broken primitive binding* dialog.
///
/// v0.9 stub: the dispatch path that detects broken bindings hasn't
/// landed yet (Stage 12+ wires the row-load checks). The handler
/// therefore only knows how to close the dialog; the actual rebind /
/// remove-row flows queue behind the detection plumbing. The dialog
/// surface itself ships now so the overlay layer is in place.
pub(super) fn handle_recovery_broken_binding(app: &mut Signex, _choice: BrokenBindingChoice) -> Task<Message> {
    app.library.recovery = None;
    Task::none()
}
