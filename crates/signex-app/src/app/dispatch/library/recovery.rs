//! Open-error recovery plumbing — the shared atomic-write helper,
//! `route_open_error` classification, and the per-choice recovery
//! actions (library-missing / git-missing / broken-binding).
//!
//! Split out of the library dispatcher (`dispatch/library`).

use super::*;

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

/// Classify a `LocalGitAdapter::open` error and, if recoverable and no
/// other recovery dialog is open, stash the matching `RecoveryDialog` on
/// `LibraryState::recovery`.
///
/// Returns `true` only when this call opened a dialog for this failure.
/// `false` means the failure produced nothing the user can see — it is
/// not recoverable, or an earlier recovery dialog is still open and must
/// not be clobbered — so the caller has to raise the error card itself.
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
) -> bool {
    // A read-only (stock) library must never reach a flow that writes
    // to it: no git re-init, no repair. The caller shows the plain
    // explanation from `open_error_notice` instead (#814).
    if state.is_read_only_target(path) {
        return false;
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
                error: None,
            })
        }
        _ => None,
    };
    // Don't clobber an already-open recovery dialog; the user resolves
    // them sequentially.
    match dialog {
        Some(d) if state.recovery.is_none() => {
            state.recovery = Some(d);
            true
        }
        _ => false,
    }
}

/// Error card for a failed library open. A read-only (stock) library
/// gets a plain explanation instead of the raw error, since no recovery
/// is offered for it.
pub(crate) fn open_error_notice(
    state: &LibraryState,
    path: &std::path::Path,
    action: &str,
    err: &LibraryError,
) -> crate::app::state::ErrorNotice {
    if state.is_read_only_target(path) {
        return read_only_repair_notice(path, &format!("could not be opened ({err})"));
    }
    crate::app::state::ErrorNotice::open(format!("Cannot {action} {}: {err}", path.display()))
}

/// Error card for a recovery attempt against a read-only (stock) library.
fn read_only_repair_notice(path: &std::path::Path, what: &str) -> crate::app::state::ErrorNotice {
    crate::app::state::ErrorNotice::open(format!(
        "The library at {} ships with Signex and {what}. \
         It is read-only and cannot be repaired from here; reinstall Signex to restore it.",
        path.display()
    ))
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

/// Result of the "Locate Library" file pick fired from the
/// *Library missing* recovery dialog. Clears the recovery state and,
/// when the user picked a replacement, re-opens the library there.
pub(super) fn handle_recovery_library_missing_locate_result(
    app: &mut Signex,
    picked: Option<std::path::PathBuf>,
) -> Task<Message> {
    app.library.recovery = None;
    if let Some(new_path) = picked {
        return Task::done(Message::Library(LibraryMessage::OpenLibraryAt(Some(
            new_path,
        ))));
    }
    Task::none()
}

/// The git re-init a *Git missing* dialog started has finished: reopen the
/// library, or keep the dialog open with the reason (#141).
pub(super) fn handle_git_reinit_finished(
    app: &mut Signex,
    path: std::path::PathBuf,
    result: Result<(), String>,
) -> Task<Message> {
    match result {
        Ok(()) => {
            app.library.recovery = None;
            Task::done(Message::Library(LibraryMessage::OpenLibraryAt(Some(path))))
        }
        Err(reason) => {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %reason,
                "git recover-init failed"
            );
            // Messages as well as the dialog: every recover-init failure is
            // logged here, a refusal (another repo's history, #1741) too.
            crate::diagnostics::log_warning(format!(
                "Re-init of {} failed: {reason}",
                path.display()
            ));
            // Keep the dialog open and show the reason so the click is
            // never a silent no-op (#141).
            if let Some(RecoveryDialog::GitMissing { error, .. }) = app.library.recovery.as_mut() {
                *error = Some(reason);
            }
            Task::none()
        }
    }
}

/// Handle the user's choice from the *Git missing* recovery dialog.
pub(super) fn handle_recovery_git_missing(
    app: &mut Signex,
    choice: GitMissingChoice,
) -> Task<Message> {
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
            if app.library.is_read_only_target(&path) {
                // Defence in depth: the dialog is never opened for a
                // read-only library, but never write to one regardless.
                app.library.recovery = None;
                app.document_state.error_notice =
                    Some(read_only_repair_notice(&path, "cannot be re-initialised"));
                return Task::none();
            }
            // A re-init already running on this library: wait for it.
            if app.library.writes.pending(&path) > 0 {
                crate::diagnostics::log_info(
                    "Re-init was ignored: this library is still being written (a re-init already running, or an edit); click it again once that has finished.",
                );
                app.sync_diagnostics_panel_ctx();
                return Task::none();
            }
            // `git init` and the first commit run on a worker (#893).
            let turn = app.library.writes.turn(&path);
            let done_path = path.clone();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        turn.wait();
                        LocalGitAdapter::recover_init(&path)
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("the git re-init task failed: {e}")))
                },
                move |result| {
                    Message::Library(LibraryMessage::GitReinitFinished {
                        path: done_path.clone(),
                        result,
                    })
                },
            )
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
pub(super) fn handle_recovery_broken_binding(
    app: &mut Signex,
    _choice: BrokenBindingChoice,
) -> Task<Message> {
    app.library.recovery = None;
    Task::none()
}
