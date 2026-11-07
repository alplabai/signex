//! Library recovery dialogs (`v0.9-snxlib-as-file-plan.md` §2 Stage H).
//!
//! When [`signex_library::LocalGitAdapter::open`] fails with a
//! recoverable error — the `.snxlib` file went missing, the `.git/`
//! directory was deleted, or a row's primitive binding points at a
//! file that's no longer on disk — the dispatcher routes the error
//! into one of the three modal flows defined here instead of merely
//! logging-and-silently-failing. The user always gets a clear choice.
//!
//! Three dialogs:
//!
//! 1. **Library missing** (`.snxlib` is gone) — *Locate…* re-opens the
//!    file picker so the user can point Signex at a moved library.
//!    *Remove from project* drops the entry from the project's
//!    `[libraries]` list.
//!
//! 2. **Git missing** (`.snxlib` is fine but `.git/` was deleted) —
//!    *Re-init* runs [`signex_library::LocalGitAdapter::recover_init`]
//!    which `git init`s a fresh repo at the parent directory and
//!    commits only the library's own files (the `.snxlib`, its
//!    folders and its ignore/attributes files) as a single
//!    "snxlib re-init" commit. Past per-primitive history is lost
//!    (the warning is bold-red on that button). *Skip (read-only)*
//!    walks away — the dispatcher just logs and keeps going.
//!    *Restore from remote* is reserved for when the manifest
//!    records a `[users.<remote>]` URL; v0.9 leaves the button
//!    disabled because no remote field exists yet (Stage 13+).
//!
//! 3. **Broken primitive binding** (a row's `symbol_uuid` /
//!    `footprint_uuid` resolves to a UUID with no on-disk file) —
//!    *Re-bind…* opens the existing primitive picker so the user
//!    can re-attach a real `.snxsym` / `.snxfpt`. *Remove row*
//!    deletes the row entirely from its table.
//!
//! Wiring lives on [`crate::library::state::LibraryState::recovery`]
//! (a single `Option<RecoveryDialog>` slot — only one dialog is
//! visible at a time). The dispatcher routes adapter errors to it;
//! the view-side overlay flow renders it from
//! `app/view/mod.rs::collect_overlays`, which runs every frame; the
//! builder's own `is_some()` guard is the only open check.

use std::path::PathBuf;

use iced::widget::{Space, column, container, text};
use iced::{Element, Widget};
use signex_library::PrimitiveKind;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use uuid::Uuid;

use super::messages::LibraryMessage;
use super::modal_chrome::{cancel_btn, danger_btn, footer_strip, header, primary_btn, secondary_btn};
use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_hint, dialog_message};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const MODAL_W: f32 = 520.0;

/// One of the three recovery flows. At most one is open at a time.
#[derive(Debug, Clone)]
pub enum RecoveryDialog {
    /// `.snxlib` file is gone — manifest can't be loaded.
    LibraryMissing { path: PathBuf },
    /// `.snxlib` file exists but the parent directory has no `.git/`.
    GitMissing {
        path: PathBuf,
        /// Optional remote URL captured from the manifest. v0.9 leaves
        /// this `None` (the manifest doesn't record one yet); Stage
        /// 13+ wires it through. The "Restore from remote" button
        /// stays disabled while this is `None`.
        remote: Option<String>,
        /// Why the last *Re-init* attempt was refused (e.g. no git
        /// identity configured); shown in the dialog, which stays open.
        error: Option<String>,
    },
    /// A row's `symbol_uuid` / `footprint_uuid` binds to a primitive
    /// UUID that has no matching on-disk file.
    BrokenPrimitiveBinding {
        library_path: PathBuf,
        table: String,
        row_id: Uuid,
        kind: PrimitiveKind,
        missing_uuid: Uuid,
    },
}

/// User choice from the *Library missing* dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryMissingChoice {
    Locate,
    RemoveFromProject,
    Cancel,
}

/// User choice from the *Git missing* dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitMissingChoice {
    ReInit,
    Skip,
    RestoreFromRemote,
    Cancel,
}

/// User choice from the *Broken primitive binding* dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokenBindingChoice {
    Rebind,
    RemoveRow,
    Cancel,
}

/// Title of the recovery dialog; the native window title of a detached one
/// reads the same.
const TITLE_LIBRARY_MISSING: &str = "Library Missing";
const TITLE_GIT_MISSING: &str = "Git Repository Missing";

pub(crate) fn title(dialog: &RecoveryDialog) -> String {
    match dialog {
        RecoveryDialog::LibraryMissing { .. } => TITLE_LIBRARY_MISSING.to_owned(),
        RecoveryDialog::GitMissing { .. } => TITLE_GIT_MISSING.to_owned(),
        RecoveryDialog::BrokenPrimitiveBinding { kind, .. } => {
            format!("Broken {} Binding", binding_kind_label(*kind))
        }
    }
}

fn binding_kind_label(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "Symbol",
        PrimitiveKind::Footprint => "Footprint",
        PrimitiveKind::Sim => "Sim Model",
        // PrimitiveKind is #[non_exhaustive]; future variants render
        // generically until the dialog gets a per-kind override.
        _ => "Primitive",
    }
}

/// Render the active recovery dialog.
pub fn view<'a>(
    dialog: &'a RecoveryDialog,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    match dialog {
        RecoveryDialog::LibraryMissing { path } => library_missing_view(path, tokens, windowed),
        RecoveryDialog::GitMissing {
            path,
            remote,
            error,
        } => git_missing_view(path, remote.as_deref(), error.as_deref(), tokens, windowed),
        RecoveryDialog::BrokenPrimitiveBinding {
            library_path,
            table,
            row_id,
            kind,
            missing_uuid,
        } => broken_binding_view(
            library_path,
            table,
            *row_id,
            *kind,
            *missing_uuid,
            tokens,
            windowed,
        ),
    }
}

fn library_missing_view<'a>(
    path: &'a std::path::Path,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let header = header(
        TITLE_LIBRARY_MISSING,
        LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
        windowed,
        tokens,
    );

    let body = column![
        dialog_message(
            "Signex couldn't find the library's .snxlib file. \
             It may have been moved, renamed, or deleted.",
            tokens,
        ),
        dialog_hint(format!("Expected: {}", path.display()), tokens),
    ]
    .spacing(SP.m);

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                "Cancel",
                LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
                tokens,
            ),
            danger_btn(
                "Remove from project",
                LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::RemoveFromProject),
                tokens,
            ),
            primary_btn(
                "Locate\u{2026}",
                LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Locate),
                tokens,
            ),
        ],
        tokens,
    );

    container(
        column![header, dialog_body(body), Space::new().height(SP.l), footer]
            .width(super::card_len(windowed, MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

fn git_missing_view<'a>(
    path: &'a std::path::Path,
    remote: Option<&'a str>,
    error: Option<&'a str>,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let danger = theme_ext::error_color(tokens);

    let header = header(
        TITLE_GIT_MISSING,
        LibraryMessage::RecoveryGitMissing(GitMissingChoice::Cancel),
        windowed,
        tokens,
    );

    let mut summary_lines = column![
        dialog_message(
            "The library's .snxlib file is intact, but its .git/ \
             directory is missing. Past per-primitive history can't \
             be read until the repository is restored.",
            tokens,
        ),
        dialog_hint(format!("Library: {}", path.display()), tokens),
    ]
    .spacing(SP.m);

    if let Some(url) = remote {
        summary_lines = summary_lines.push(dialog_hint(format!("Remote: {}", url), tokens));
    }

    if let Some(reason) = error {
        summary_lines = summary_lines.push(
            text(format!("Re-init failed: {reason}"))
                .size(TS.body)
                .color(danger)
                .boxed(),
        );
    }

    summary_lines = summary_lines.push(
        text(
            "Re-init creates a fresh repository at the parent \
             directory. Past commits are lost; new edits are tracked \
             from this point forward.",
        )
        .size(TS.body)
        .color(danger)
        .boxed(),
    );

    // Restoring is the affirmative action once a remote is known; until
    // then it is drawn disabled.
    let restore_button: Element<'a, LibraryMessage> = if remote.is_some() {
        primary_btn(
            "Restore from remote",
            LibraryMessage::RecoveryGitMissing(GitMissingChoice::RestoreFromRemote),
            tokens,
        )
    } else {
        dialog_button(
            "Restore from remote",
            DialogButtonKind::Secondary,
            None,
            tokens,
        )
    };

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                "Cancel",
                LibraryMessage::RecoveryGitMissing(GitMissingChoice::Cancel),
                tokens,
            ),
            secondary_btn(
                "Skip (read-only)",
                LibraryMessage::RecoveryGitMissing(GitMissingChoice::Skip),
                tokens,
            ),
            restore_button,
            danger_btn(
                "Re-init (lose history)",
                LibraryMessage::RecoveryGitMissing(GitMissingChoice::ReInit),
                tokens,
            ),
        ],
        tokens,
    );

    container(
        column![
            header,
            dialog_body(summary_lines),
            Space::new().height(SP.l),
            footer
        ]
        .width(super::card_len(windowed, MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

fn broken_binding_view<'a>(
    library_path: &'a std::path::Path,
    table: &'a str,
    row_id: Uuid,
    kind: PrimitiveKind,
    missing_uuid: Uuid,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let kind_label = binding_kind_label(kind);

    let header = header(
        &format!("Broken {} Binding", kind_label),
        LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Cancel),
        windowed,
        tokens,
    );

    let body = column![
        dialog_message(
            format!(
                "A row in '{}' references a {} that no longer exists on disk.",
                table,
                kind_label.to_lowercase()
            ),
            tokens,
        ),
        column![
            dialog_hint(format!("Library: {}", library_path.display()), tokens),
            dialog_hint(format!("Row: {}", row_id), tokens),
            dialog_hint(format!("Missing UUID: {}", missing_uuid), tokens),
        ]
        .spacing(SP.xs),
    ]
    .spacing(SP.m);

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                "Cancel",
                LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Cancel),
                tokens,
            ),
            danger_btn(
                "Remove row",
                LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::RemoveRow),
                tokens,
            ),
            primary_btn(
                "Re-bind\u{2026}",
                LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Rebind),
                tokens,
            ),
        ],
        tokens,
    );

    container(
        column![header, dialog_body(body), Space::new().height(SP.l), footer]
            .width(super::card_len(windowed, MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn library_missing_carries_path() {
        let d = RecoveryDialog::LibraryMissing {
            path: PathBuf::from("/tmp/foo.snxlib"),
        };
        match d {
            RecoveryDialog::LibraryMissing { path } => {
                assert_eq!(path, PathBuf::from("/tmp/foo.snxlib"));
            }
            _ => panic!("variant mismatch"),
        }
    }

    #[test]
    fn git_missing_remote_defaults_none() {
        let d = RecoveryDialog::GitMissing {
            path: PathBuf::from("/tmp/foo.snxlib"),
            remote: None,
            error: None,
        };
        match d {
            RecoveryDialog::GitMissing { remote, .. } => assert!(remote.is_none()),
            _ => panic!("variant mismatch"),
        }
    }

    #[test]
    fn broken_binding_carries_kind_and_uuids() {
        let lib = PathBuf::from("/tmp/foo.snxlib");
        let row = Uuid::nil();
        let missing = Uuid::nil();
        let d = RecoveryDialog::BrokenPrimitiveBinding {
            library_path: lib.clone(),
            table: "resistors".into(),
            row_id: row,
            kind: PrimitiveKind::Symbol,
            missing_uuid: missing,
        };
        match d {
            RecoveryDialog::BrokenPrimitiveBinding {
                library_path,
                table,
                kind,
                ..
            } => {
                assert_eq!(library_path, lib);
                assert_eq!(table, "resistors");
                assert!(matches!(kind, PrimitiveKind::Symbol));
            }
            _ => panic!("variant mismatch"),
        }
    }
}
