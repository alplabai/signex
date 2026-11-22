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
/// Missing-file recovery keys on `LibraryError::LibraryFileMissing`
/// (the file is confirmed absent), never on message text: Remove from
/// project must not be offered for a library that exists but failed to
/// open for another reason.
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
        LibraryError::LibraryFileMissing { .. } => Some(RecoveryDialog::LibraryMissing {
            path: path.to_path_buf(),
        }),
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
    action: signex_i18n::MessageId,
    err: &LibraryError,
) -> crate::app::state::ErrorNotice {
    let detail = if state.is_read_only_target(path) {
        signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_STOCK_LIBRARY_NOT_OPENED)
    } else {
        signex_i18n::Translatable::new(action)
    };
    crate::app::state::ErrorNotice::open(
        detail
            .arg("path", path.display().to_string())
            .arg("error", err.to_string()),
    )
}

/// Error card for a recovery attempt against a read-only (stock) library.
fn read_only_repair_notice(path: &std::path::Path) -> crate::app::state::ErrorNotice {
    crate::app::state::ErrorNotice::open(
        signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_STOCK_LIBRARY_NOT_REINITIALISED)
            .arg("path", path.display().to_string()),
    )
}

impl Signex {
    /// File name of the project that lists the library the open Library
    /// Missing dialog is about; the dialog's header shows it.
    pub(crate) fn recovery_project_file(&self) -> Option<String> {
        let owner = self.library.missing.open_owner()?;
        let project = self.document_state.project_by_id(owner)?;
        project
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }
}

/// Handle the user's choice from the *Library missing* recovery dialog.
pub(super) fn handle_recovery_library_missing(
    app: &mut Signex,
    choice: LibraryMissingChoice,
) -> Task<Message> {
    match choice {
        LibraryMissingChoice::Cancel => {
            app.library.finish_recovery();
            Task::none()
        }
        LibraryMissingChoice::Locate => {
            // One pick at a time: a second click while the picker is open
            // would land a result for a dialog that has moved on.
            if app.library.missing.is_locating() {
                return Task::none();
            }
            if let Some(RecoveryDialog::LibraryMissing { path }) = app.library.recovery.as_ref() {
                let path = path.clone();
                app.library.missing.start_locating(path);
            }
            let i18n = &app.ui_state.i18n;
            let dialog = app
                .ui_state
                .io
                .picker
                .dialog()
                .title_in(i18n, signex_i18n::ids::FILE_DIALOG_LOCATE_LIBRARY)
                .filter_in(i18n, signex_i18n::ids::FILE_FILTER_LIBRARY, &["snxlib"]);
            Task::perform(async move { dialog.pick_file().await }, |path| {
                Message::Library(LibraryMessage::RecoveryLibraryMissingLocateResult(path))
            })
        }
        LibraryMissingChoice::RemoveFromProject => {
            let missing = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::LibraryMissing { path }) => path.clone(),
                _ => {
                    app.library.finish_recovery();
                    return Task::none();
                }
            };
            let mut changed = Vec::new();
            for project in app.document_state.projects.iter_mut() {
                let before = project.data.libraries.len();
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
                if project.data.libraries.len() != before {
                    changed.push(project.path.clone());
                }
            }
            // The change is unsaved until the user saves the project.
            for path in &changed {
                app.document_state.mark_document_dirty(path);
            }
            app.library.finish_recovery();
            Task::none()
        }
    }
}

/// Result of the "Locate Library" file pick fired from the
/// *Library missing* recovery dialog. Opens the picked library and, only
/// once it has mounted, repoints the project entries that listed the
/// missing path. A pick that comes back after its dialog moved on only
/// opens the library.
pub(super) fn handle_recovery_library_missing_locate_result(
    app: &mut Signex,
    picked: Option<std::path::PathBuf>,
) -> Task<Message> {
    let located = app.library.missing.take_locating();
    let dialog_path = match app.library.recovery.as_ref() {
        Some(RecoveryDialog::LibraryMissing { path }) => Some(path.clone()),
        _ => None,
    };
    let still_open = located.is_some() && located == dialog_path;
    let Some(new_path) = picked else {
        // Picker cancelled: the dialog stays for another choice.
        return Task::none();
    };
    if still_open {
        // Closed without moving on yet: a failed open may raise its own
        // recovery dialog, which must not be replaced by the next queued one.
        app.library.recovery = None;
    }
    // The picked library takes the missing one's place in its projects, so
    // it ranks as theirs while it mounts.
    let replaces_project_entry = still_open
        && located
            .as_deref()
            .is_some_and(|old| app.open_project_library_paths().iter().any(|p| p == old));
    let task = app.open_library_at_as(new_path.clone(), replaces_project_entry);
    if still_open {
        if let Some(old) = located.as_deref()
            && app.library.library_at(&new_path).is_some()
        {
            relocate_project_entries(app, old, &new_path);
        }
        if app.library.recovery.is_none() {
            app.library.finish_recovery();
        }
    }
    task
}

impl Signex {
    /// Locate… on a Project tab's Missing library row (mockup row 33): mount
    /// `picked` and, only once it has mounted, repoint the entries that
    /// listed `missing`. A Library Missing dialog for it has nothing left to
    /// ask. A pick that does not open raises its own dialog or error card.
    pub(crate) fn repoint_missing_library(
        &mut self,
        missing: &std::path::Path,
        picked: std::path::PathBuf,
    ) -> Task<Message> {
        let task = self.open_library_at_as(picked.clone(), true);
        if self.library.library_at(&picked).is_some() {
            relocate_project_entries(self, missing, &picked);
            self.library.forget_missing_library(missing);
        }
        task
    }
}

/// Point every project entry that listed `old` at `new`, and mark those
/// projects dirty. Unsaved until the user saves the project.
fn relocate_project_entries(app: &mut Signex, old: &std::path::Path, new: &std::path::Path) {
    use crate::app::state::library_entry::library_entry_location;
    use signex_types::project::LibraryEntryKind;
    let mut changed = Vec::new();
    for project in app.document_state.projects.iter_mut() {
        let dir = std::path::PathBuf::from(&project.data.dir);
        let fold = project.case_fold;
        let (path, kind) = library_entry_location(&dir, new, fold);
        let hits: Vec<usize> = project
            .data
            .libraries
            .iter()
            .enumerate()
            .filter(|(_, e)| project.data.resolve_library_path(e) == old)
            .map(|(i, _)| i)
            .collect();
        if hits.is_empty() {
            continue;
        }
        changed.push(project.path.clone());
        let lists_new = project
            .data
            .libraries
            .iter()
            .any(|e| project.data.resolve_library_path(e) == new);
        if lists_new {
            // Already listed at the new place: drop the stale entries
            // instead of making a second entry for the same library.
            let mut idx = 0usize;
            project.data.libraries.retain(|_| {
                let keep = !hits.contains(&idx);
                idx += 1;
                keep
            });
            continue;
        }
        for &i in &hits {
            let entry = &mut project.data.libraries[i];
            // A Global entry stays Global when the new place is outside
            // the project.
            entry.kind = match (entry.kind, kind) {
                (LibraryEntryKind::Global, LibraryEntryKind::Shared) => LibraryEntryKind::Global,
                (_, kind) => kind,
            };
            entry.path = path.clone();
        }
    }
    for path in &changed {
        app.document_state.mark_document_dirty(path);
    }
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
            app.library.finish_recovery();
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
        // The close (x, Esc) leaves the library unopened.
        GitMissingChoice::Cancel => {
            app.library.finish_recovery();
            Task::none()
        }
        GitMissingChoice::Skip => open_without_history(app),
        GitMissingChoice::ReInit => {
            let path = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::GitMissing { path, .. }) => path.clone(),
                _ => {
                    app.library.finish_recovery();
                    return Task::none();
                }
            };
            if app.library.is_read_only_target(&path) {
                // Defence in depth: the dialog is never opened for a
                // read-only library, but never write to one regardless.
                app.library.finish_recovery();
                app.document_state.error_notice = Some(read_only_repair_notice(&path));
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
            // remote yet. Closes the dialog.
            app.library.finish_recovery();
            Task::none()
        }
    }
}

/// Skip for now on *Git missing*: close the dialog and open the library
/// read-only until its history is restored (Caner, 2026-11-20; mockup row
/// 138). The mount is prepared off the UI thread and lands as
/// `MountFinished`, like any browser open.
fn open_without_history(app: &mut Signex) -> Task<Message> {
    use crate::library::mount::{MountIntent, MountRequest};
    let path = match app.library.recovery.as_ref() {
        Some(RecoveryDialog::GitMissing { path, .. }) => Some(path.clone()),
        _ => None,
    };
    app.library.finish_recovery();
    let Some(path) = path else {
        return Task::none();
    };
    match app
        .library
        .request_mount(&path, MountIntent::OpenBrowserTab)
    {
        MountRequest::AlreadyMounted => app.finish_open_library_browser(path),
        // An open of this path is already preparing; it lands on its own.
        MountRequest::InFlight => Task::none(),
        MountRequest::Spawn => {
            let spawn_path = path.clone();
            Task::perform(
                crate::library::mount::prepare_mount_without_history_off_thread(spawn_path),
                move |prepared| {
                    Message::Library(LibraryMessage::MountFinished {
                        path: path.clone(),
                        prepared,
                    })
                },
            )
        }
    }
}

/// Handle the user's choice from the *Broken primitive binding* dialog
/// (mockup row 138). Re-bind and Remove row go back through the router, so
/// they meet the same read-only and library-rename refusals as the Browser's
/// own Pick and Delete.
pub(super) fn handle_recovery_broken_binding(
    app: &mut Signex,
    choice: BrokenBindingChoice,
) -> Task<Message> {
    let binding = match app.library.recovery.as_ref() {
        Some(RecoveryDialog::BrokenPrimitiveBinding {
            library_path,
            table,
            row_id,
            kind,
            ..
        }) => Some((
            library_path.clone(),
            table.clone(),
            RowId::from_uuid(*row_id),
            *kind,
        )),
        _ => None,
    };
    app.library.finish_recovery();
    let Some((library_path, table, row_id, kind)) = binding else {
        return Task::none();
    };
    let next = match choice {
        BrokenBindingChoice::Cancel => return Task::none(),
        BrokenBindingChoice::Rebind => LibraryMessage::OpenPrimitivePicker {
            kind,
            target: PrimitivePickerTarget::BrowserRow(EditorAddress::new(
                library_path,
                table,
                row_id,
            )),
        },
        BrokenBindingChoice::RemoveRow => LibraryMessage::BrowserDeleteRowConfirm {
            library_path,
            table,
            row_id,
        },
    };
    Task::done(Message::Library(next))
}
