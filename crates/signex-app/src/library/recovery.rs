//! Library recovery dialogs (`v0.9-snxlib-as-file-plan.md` §2 Stage H).
//!
//! When [`signex_library::LocalGitAdapter::open`] fails with a
//! recoverable error — the `.snxlib` file went missing, the `.git/`
//! directory was deleted, or a row's primitive binding points at a
//! file that's no longer on disk — the dispatcher routes the error
//! into one of the three modal flows defined here instead of merely
//! logging-and-silently-failing. The user always gets a clear choice.
//!
//! The three share one layout: an intro with a glyph, the facts in a
//! key / value box, a caption saying what Skip for now leaves behind, and a
//! footer with the destructive action on the left, then Skip for now, then
//! the fix as the primary ([`footer_plan`]).
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
//!    "snxlib re-init" commit. Past per-primitive history is lost,
//!    which a banner says. *Skip for now* opens the library read-only
//!    until its history is restored: every edit is refused with the
//!    reason. The close (x, Esc) leaves the library unopened.
//!    *Restore from remote* is reserved for when the
//!    manifest records a `[users.<remote>]` URL; v0.9 leaves the button
//!    disabled because no remote field exists yet (Stage 13+). While it is
//!    disabled it stands before Skip for now, which is the primary (mockup
//!    row 138).
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

use std::path::{Path, PathBuf};

use iced::widget::{Column, Space, column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_library::PrimitiveKind;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;
use uuid::Uuid;

use super::messages::LibraryMessage;
use super::modal_chrome::{
    footer_strip, header_with_note_closing, key_value_box, mono_value, notice_banner,
};
use crate::styles::{
    DialogButtonKind, DialogIcon, PillTone, Spacing, TypeScale, dialog_body, dialog_button,
    dialog_intro, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the dialog card in the main window.
pub(crate) const MODAL_W: f32 = 560.0;

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
        /// The row's internal part number, when known; the dialog names the
        /// row by it, else by `row_id`.
        internal_pn: Option<String>,
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
pub(crate) fn title(dialog: &RecoveryDialog, i18n: &I18n) -> String {
    match dialog {
        RecoveryDialog::LibraryMissing { .. } => {
            i18n.message_plain(ids::LIBRARY_RECOVERY_MISSING_TITLE, &[])
        }
        RecoveryDialog::GitMissing { .. } => {
            i18n.message_plain(ids::LIBRARY_RECOVERY_GIT_TITLE, &[])
        }
        RecoveryDialog::BrokenPrimitiveBinding { kind, .. } => i18n.message_plain(
            ids::LIBRARY_RECOVERY_BINDING_TITLE,
            &[("kind", signex_i18n::Arg::from(kind_arg(*kind)))],
        ),
    }
}

/// The `$kind` selector of the binding messages.
fn kind_arg(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "symbol",
        PrimitiveKind::Footprint => "footprint",
        PrimitiveKind::Sim => "sim",
        // PrimitiveKind is #[non_exhaustive]; future variants read
        // generically until the catalog gets a per-kind entry.
        _ => "other",
    }
}

/// The library's name as the dialogs show it: its file stem.
fn library_name(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The muted note in the header: the project file that lists a missing
/// library (`project_file`, when known), or the library a history or
/// binding dialog is about.
pub(crate) fn note(dialog: &RecoveryDialog, project_file: Option<String>) -> Option<String> {
    match dialog {
        RecoveryDialog::LibraryMissing { .. } => project_file,
        RecoveryDialog::GitMissing { path, .. } => Some(library_name(path)),
        RecoveryDialog::BrokenPrimitiveBinding { library_path, .. } => {
            Some(library_name(library_path))
        }
    }
}

/// The intro's title and secondary text.
pub(crate) fn intro_texts(dialog: &RecoveryDialog, i18n: &I18n) -> (String, String) {
    match dialog {
        RecoveryDialog::BrokenPrimitiveBinding { table, kind, .. } => {
            binding_intro_texts(i18n, *kind, table)
        }
        _ => {
            let (title, secondary) = match dialog {
                RecoveryDialog::LibraryMissing { .. } => (
                    ids::LIBRARY_RECOVERY_MISSING_INTRO_TITLE,
                    ids::LIBRARY_RECOVERY_MISSING_INTRO_TEXT,
                ),
                _ => (
                    ids::LIBRARY_RECOVERY_GIT_INTRO_TITLE,
                    ids::LIBRARY_RECOVERY_GIT_INTRO_TEXT,
                ),
            };
            (
                t!(i18n, title).into_string(),
                t!(i18n, secondary).into_string(),
            )
        }
    }
}

/// [`intro_texts`] with a one-letter table name: the texts the window
/// height was measured with.
pub(crate) fn reference_intro_texts(dialog: &RecoveryDialog, i18n: &I18n) -> (String, String) {
    match dialog {
        RecoveryDialog::BrokenPrimitiveBinding { kind, .. } => {
            binding_intro_texts(i18n, *kind, "x")
        }
        _ => intro_texts(dialog, i18n),
    }
}

fn binding_intro_texts(i18n: &I18n, kind: PrimitiveKind, table: &str) -> (String, String) {
    (
        t!(i18n, ids::LIBRARY_RECOVERY_BINDING_INTRO_TITLE, "kind" => kind_arg(kind)).into_string(),
        t!(
            i18n,
            ids::LIBRARY_RECOVERY_BINDING_INTRO_TEXT,
            "kind" => kind_arg(kind),
            "table" => table.to_owned()
        )
        .into_string(),
    )
}

/// The caption under the facts: what Skip for now leaves behind.
pub(crate) fn caption(dialog: &RecoveryDialog, i18n: &I18n) -> Localized {
    match dialog {
        RecoveryDialog::LibraryMissing { path } => t!(
            i18n,
            ids::LIBRARY_RECOVERY_MISSING_CAPTION,
            "library" => library_name(path)
        ),
        RecoveryDialog::GitMissing { .. } => t!(i18n, ids::LIBRARY_RECOVERY_GIT_CAPTION),
        RecoveryDialog::BrokenPrimitiveBinding { .. } => {
            t!(i18n, ids::LIBRARY_RECOVERY_BINDING_CAPTION)
        }
    }
}

/// The banners between the facts and the caption, in order: a failed
/// re-init, then the fixed warning that re-init loses the history.
pub(crate) fn banners(dialog: &RecoveryDialog, i18n: &I18n) -> Vec<Localized> {
    match dialog {
        RecoveryDialog::GitMissing { error, .. } => {
            let failed = error.as_ref().map(|reason| {
                t!(
                    i18n,
                    ids::LIBRARY_RECOVERY_GIT_REINIT_FAILED,
                    "error" => reason.clone()
                )
            });
            failed
                .into_iter()
                .chain(std::iter::once(t!(
                    i18n,
                    ids::LIBRARY_RECOVERY_GIT_REINIT_WARNING
                )))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Number of key / value rows in the facts box.
pub(crate) fn fact_rows(dialog: &RecoveryDialog) -> usize {
    match dialog {
        RecoveryDialog::GitMissing { remote: None, .. } => 1,
        _ => 2,
    }
}

/// Render the active recovery dialog. `project_file` names the project
/// that lists a missing library, when known.
pub fn view<'a>(
    dialog: &'a RecoveryDialog,
    project_file: Option<String>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let (cancel, icon) = match dialog {
        RecoveryDialog::LibraryMissing { .. } => (
            LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
            DialogIcon::Folder,
        ),
        RecoveryDialog::GitMissing { .. } => (
            LibraryMessage::RecoveryGitMissing(GitMissingChoice::Cancel),
            DialogIcon::Git,
        ),
        RecoveryDialog::BrokenPrimitiveBinding { .. } => (
            LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Cancel),
            DialogIcon::Warn,
        ),
    };
    let header = header_with_note_closing(
        &title(dialog, i18n),
        note(dialog, project_file),
        cancel,
        windowed,
        tokens,
    );

    let (intro_title, intro_text) = intro_texts(dialog, i18n);
    let mut body: Column<Element<'a, LibraryMessage>> = column![
        dialog_intro(icon, intro_title, Some(intro_text), tokens),
        key_value_box(facts(dialog, i18n, tokens), tokens),
    ]
    .spacing(SP.m);
    for banner in banners(dialog, i18n) {
        body = body.push(notice_banner(
            PillTone::Err,
            text(banner).size(TS.body).boxed(),
            Vec::new(),
            tokens,
        ));
    }
    body = body.push(
        text(caption(dialog, i18n))
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink4)
            .boxed(),
    );

    // As tall as the content in the main window; in its own window (sized
    // to the content) the footer stays at the bottom.
    let mut card = column![header, dialog_body(body), Space::new().height(SP.l)];
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    container(
        card.push(footer(dialog, i18n, tokens))
            .width(super::card_len(windowed, MODAL_W))
            .height(if windowed {
                Length::Fill
            } else {
                Length::Shrink
            }),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// The facts box rows of `dialog`.
fn facts<'a>(
    dialog: &RecoveryDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Vec<(Localized, Element<'a, LibraryMessage>)> {
    let plain = |value: String| text(UserText::new(value)).size(TS.body).boxed();
    match dialog {
        RecoveryDialog::LibraryMissing { path } => vec![
            (
                t!(i18n, ids::LIBRARY_RECOVERY_KEY_LIBRARY),
                plain(library_name(path)),
            ),
            (
                t!(i18n, ids::LIBRARY_RECOVERY_KEY_EXPECTED),
                mono_value(path.display().to_string(), tokens),
            ),
        ],
        RecoveryDialog::GitMissing { path, remote, .. } => {
            let mut rows = vec![(
                t!(i18n, ids::LIBRARY_RECOVERY_KEY_LIBRARY),
                plain(library_name(path)),
            )];
            if let Some(url) = remote {
                rows.push((
                    t!(i18n, ids::LIBRARY_RECOVERY_KEY_REMOTE),
                    mono_value(url.clone(), tokens),
                ));
            }
            rows
        }
        RecoveryDialog::BrokenPrimitiveBinding {
            row_id,
            internal_pn,
            missing_uuid,
            ..
        } => vec![
            (
                t!(i18n, ids::LIBRARY_RECOVERY_KEY_ROW),
                mono_value(
                    internal_pn.clone().unwrap_or_else(|| row_id.to_string()),
                    tokens,
                ),
            ),
            (
                t!(i18n, ids::LIBRARY_RECOVERY_KEY_MISSING_UUID),
                mono_value(missing_uuid.to_string(), tokens),
            ),
        ],
    }
}

/// Whether the fix can be pressed: Restore from remote needs a known
/// remote; the other fixes always can.
pub(crate) fn fix_enabled(dialog: &RecoveryDialog) -> bool {
    !matches!(dialog, RecoveryDialog::GitMissing { remote: None, .. })
}

/// One footer button: its label, its role, and what a press sends (`None`
/// draws it disabled).
#[derive(Debug, Clone)]
pub(crate) struct FooterButton {
    pub label: MessageId,
    pub kind: DialogButtonKind,
    pub message: Option<LibraryMessage>,
}

/// The footer of `dialog`: the destructive action for the left end, and
/// the right-aligned buttons with the one primary last (DECISIONS
/// 2026-11-08). That is Skip for now, then the fix as the primary. While
/// the fix cannot be pressed it is drawn disabled before Skip for now, and
/// Skip for now is the primary, so the dialog keeps a default action
/// (mockup row 138).
pub(crate) fn footer_plan(dialog: &RecoveryDialog) -> (FooterButton, Vec<FooterButton>) {
    let (destroy, skip_message, (fix_label, fix_message)) = footer_actions(dialog);
    let skip = |kind| FooterButton {
        label: ids::LIBRARY_RECOVERY_SKIP_FOR_NOW,
        kind,
        message: Some(skip_message.clone()),
    };
    let fix = |kind| FooterButton {
        label: fix_label,
        kind,
        message: fix_message.clone(),
    };
    let right = if fix_enabled(dialog) {
        vec![
            skip(DialogButtonKind::Secondary),
            fix(DialogButtonKind::Primary),
        ]
    } else {
        vec![
            fix(DialogButtonKind::Secondary),
            skip(DialogButtonKind::Primary),
        ]
    };
    let destroy = FooterButton {
        label: destroy.0,
        kind: DialogButtonKind::Danger,
        message: Some(destroy.1),
    };
    (destroy, right)
}

/// The destructive action, what Skip for now sends, and the fix of
/// `dialog`; Restore from remote has no message until a remote is known.
fn footer_actions(
    dialog: &RecoveryDialog,
) -> (
    (MessageId, LibraryMessage),
    LibraryMessage,
    (MessageId, Option<LibraryMessage>),
) {
    use LibraryMessage::{RecoveryBrokenBinding, RecoveryGitMissing, RecoveryLibraryMissing};
    match dialog {
        RecoveryDialog::LibraryMissing { .. } => (
            (
                ids::LIBRARY_RECOVERY_REMOVE_FROM_PROJECT,
                RecoveryLibraryMissing(LibraryMissingChoice::RemoveFromProject),
            ),
            RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
            (
                ids::LIBRARY_RECOVERY_LOCATE,
                Some(RecoveryLibraryMissing(LibraryMissingChoice::Locate)),
            ),
        ),
        RecoveryDialog::GitMissing { remote, .. } => (
            (
                ids::LIBRARY_RECOVERY_REINIT,
                RecoveryGitMissing(GitMissingChoice::ReInit),
            ),
            RecoveryGitMissing(GitMissingChoice::Skip),
            (
                ids::LIBRARY_RECOVERY_RESTORE_REMOTE,
                remote
                    .as_ref()
                    .map(|_| RecoveryGitMissing(GitMissingChoice::RestoreFromRemote)),
            ),
        ),
        RecoveryDialog::BrokenPrimitiveBinding { .. } => (
            (
                ids::LIBRARY_RECOVERY_REMOVE_ROW,
                RecoveryBrokenBinding(BrokenBindingChoice::RemoveRow),
            ),
            RecoveryBrokenBinding(BrokenBindingChoice::Cancel),
            (
                ids::LIBRARY_RECOVERY_REBIND,
                Some(RecoveryBrokenBinding(BrokenBindingChoice::Rebind)),
            ),
        ),
    }
}

/// The footer as [`footer_plan`] lays it out.
fn footer<'a>(
    dialog: &RecoveryDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (destroy, right) = footer_plan(dialog);
    let draw = |button: FooterButton| {
        dialog_button(t!(i18n, button.label), button.kind, button.message, tokens)
    };
    let left = vec![draw(destroy)];
    footer_strip(left, right.into_iter().map(draw).collect(), tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;
    use std::path::PathBuf;

    fn binding(kind: PrimitiveKind, internal_pn: Option<&str>) -> RecoveryDialog {
        RecoveryDialog::BrokenPrimitiveBinding {
            library_path: PathBuf::from("/libs/passives/passives.snxlib"),
            table: "resistors".into(),
            row_id: Uuid::nil(),
            internal_pn: internal_pn.map(str::to_owned),
            kind,
            missing_uuid: Uuid::nil(),
        }
    }

    fn all_dialogs() -> Vec<RecoveryDialog> {
        vec![
            RecoveryDialog::LibraryMissing {
                path: PathBuf::from("/libs/power-ics/power-ics.snxlib"),
            },
            RecoveryDialog::GitMissing {
                path: PathBuf::from("/libs/passives/passives.snxlib"),
                remote: None,
                error: None,
            },
            RecoveryDialog::GitMissing {
                path: PathBuf::from("/libs/passives/passives.snxlib"),
                remote: Some("git@example.com:hw/passives.git".into()),
                error: Some("no git identity".into()),
            },
            binding(PrimitiveKind::Footprint, Some("R-0805-1K-1%")),
            binding(PrimitiveKind::Symbol, None),
        ]
    }

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
            internal_pn: None,
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

    /// The titles come from the catalog and read as before.
    #[test]
    fn the_titles_read_as_the_dialogs_name_them() {
        let i18n = I18n::en_us();
        let dialogs = all_dialogs();
        assert_eq!(title(&dialogs[0], &i18n), "Library Missing");
        assert_eq!(title(&dialogs[1], &i18n), "Git Repository Missing");
        assert_eq!(title(&dialogs[3], &i18n), "Broken Footprint Binding");
        assert_eq!(title(&dialogs[4], &i18n), "Broken Symbol Binding");
    }

    /// The header note names the library a history or binding dialog is
    /// about; a missing library's note is the project file, when known.
    #[test]
    fn the_note_names_the_project_or_the_library() {
        let dialogs = all_dialogs();
        assert_eq!(note(&dialogs[0], None), None);
        assert_eq!(
            note(&dialogs[0], Some("dcdc.snxprj".into())).as_deref(),
            Some("dcdc.snxprj")
        );
        assert_eq!(note(&dialogs[1], None).as_deref(), Some("passives"));
        assert_eq!(note(&dialogs[3], None).as_deref(), Some("passives"));
    }

    /// The facts box has a Remote row only when a remote is known; a failed
    /// re-init adds a banner above the fixed history warning.
    #[test]
    fn the_git_dialog_shows_remote_and_failure_only_when_known() {
        let i18n = I18n::en_us();
        let dialogs = all_dialogs();
        assert_eq!(fact_rows(&dialogs[1]), 1);
        assert_eq!(fact_rows(&dialogs[2]), 2);
        assert_eq!(banners(&dialogs[1], &i18n).len(), 1);
        let shown: Vec<String> = banners(&dialogs[2], &i18n)
            .into_iter()
            .map(Localized::into_string)
            .collect();
        assert_eq!(shown.len(), 2);
        assert!(shown[0].contains("no git identity"), "{shown:?}");
        assert!(
            shown[1].contains("all earlier commits are lost"),
            "{shown:?}"
        );
        assert!(banners(&dialogs[0], &i18n).is_empty());
    }

    /// `(label, role, can be pressed)` of the right-aligned footer buttons.
    fn right_shape(dialog: &RecoveryDialog) -> Vec<(MessageId, DialogButtonKind, bool)> {
        footer_plan(dialog)
            .1
            .iter()
            .map(|b| (b.label, b.kind, b.message.is_some()))
            .collect()
    }

    /// Mockup row 138 (layout): the fix is the one primary, last, after a
    /// secondary Skip for now. With no remote, the disabled Restore from
    /// remote is secondary and stands first, and Skip for now is the
    /// primary at the right end; the built footer draws them in that order.
    #[test]
    fn skip_for_now_is_the_primary_while_restore_from_remote_is_disabled() {
        use crate::styles::DialogButtonKind::{Danger, Primary, Secondary};
        let skip = ids::LIBRARY_RECOVERY_SKIP_FOR_NOW;
        let dialogs = all_dialogs();
        let no_remote = &dialogs[1];
        assert!(!fix_enabled(no_remote));
        assert_eq!(
            right_shape(no_remote),
            vec![
                (ids::LIBRARY_RECOVERY_RESTORE_REMOTE, Secondary, false),
                (skip, Primary, true),
            ]
        );
        assert!(matches!(
            footer_plan(no_remote).1[1].message,
            Some(LibraryMessage::RecoveryGitMissing(GitMissingChoice::Skip))
        ));
        let fixes = [
            (&dialogs[0], ids::LIBRARY_RECOVERY_LOCATE),
            (&dialogs[2], ids::LIBRARY_RECOVERY_RESTORE_REMOTE),
            (&dialogs[3], ids::LIBRARY_RECOVERY_REBIND),
            (&dialogs[4], ids::LIBRARY_RECOVERY_REBIND),
        ];
        for (dialog, fix) in fixes {
            assert!(fix_enabled(dialog), "{dialog:?}");
            let expected = vec![(skip, Secondary, true), (fix, Primary, true)];
            assert_eq!(right_shape(dialog), expected, "{dialog:?}");
        }
        for dialog in &dialogs {
            assert_eq!(footer_plan(dialog).0.kind, Danger, "{dialog:?}");
        }

        let i18n = I18n::en_us();
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let window = iced::Size::new(MODAL_W, 2000.0);
        let texts = crate::layout_probe::lay_out_texts(
            view(no_remote, None, &tokens, &i18n, false),
            window,
        );
        let x = |label: &str| crate::layout_probe::find(&texts, label).bounds.x;
        assert!(
            x("Restore from remote") < x("Skip for now"),
            "the primary Skip for now is the last button"
        );
    }

    /// Every case draws from tokens alone in every built-in theme, in the
    /// main window and in its own.
    #[test]
    fn every_case_draws_in_every_theme() {
        let i18n = I18n::en_us();
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            for dialog in &all_dialogs() {
                for windowed in [false, true] {
                    drop(view(
                        dialog,
                        Some("dcdc.snxprj".into()),
                        &tokens,
                        &i18n,
                        windowed,
                    ));
                }
            }
        }
    }
}
