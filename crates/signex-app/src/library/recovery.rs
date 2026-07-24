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
//!    stages the current working tree as a single
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
//! `app/view/mod.rs::collect_overlays`. The overlay predicate in the
//! same file gates whether `collect_overlays` runs at all (see the
//! `[needs_overlay predicate gates modal rendering]` invariant —
//! without that flag the modal just doesn't paint and clicks
//! vanish).

use std::path::PathBuf;

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_library::PrimitiveKind;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use uuid::Uuid;

use super::messages::LibraryMessage;

const MODAL_W: f32 = 520.0;

/// One of the three recovery flows. At most one is open at a time.
#[derive(Debug, Clone)]
#[allow(dead_code)]
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
#[allow(dead_code)]
pub enum LibraryMissingChoice {
    Locate,
    RemoveFromProject,
    Cancel,
}

/// User choice from the *Git missing* dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum GitMissingChoice {
    ReInit,
    Skip,
    RestoreFromRemote,
    Cancel,
}

/// User choice from the *Broken primitive binding* dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BrokenBindingChoice {
    Rebind,
    RemoveRow,
    Cancel,
}

/// Render the active recovery dialog.
pub fn view<'a>(
    dialog: &'a RecoveryDialog,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    match dialog {
        RecoveryDialog::LibraryMissing { path } => library_missing_view(path, tokens),
        RecoveryDialog::GitMissing { path, remote } => {
            git_missing_view(path, remote.as_deref(), tokens)
        }
        RecoveryDialog::BrokenPrimitiveBinding {
            library_path,
            table,
            row_id,
            kind,
            missing_uuid,
        } => broken_binding_view(library_path, table, *row_id, *kind, *missing_uuid, tokens),
    }
}

fn library_missing_view<'a>(
    path: &'a std::path::Path,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("Library Missing").size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
                tokens,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let body = column![
        text(
            "Signex couldn't find the library's .snxlib file. \
             It may have been moved, renamed, or deleted."
        )
        .size(11)
        .color(muted),
        Space::new().height(8),
        text(format!("Expected: {}", path.display()))
            .size(11)
            .color(muted),
    ]
    .spacing(2);

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            secondary_btn(
                "Cancel",
                LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
                text_c,
                border,
            ),
            Space::new().width(8),
            secondary_btn(
                "Remove from project",
                LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::RemoveFromProject),
                text_c,
                border,
            ),
            Space::new().width(8),
            primary_btn(
                "Locate\u{2026}",
                LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Locate),
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, container(body).padding([14, 16]), footer].width(Length::Fixed(MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}

fn git_missing_view<'a>(
    path: &'a std::path::Path,
    remote: Option<&'a str>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("Git Repository Missing").size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::RecoveryGitMissing(GitMissingChoice::Cancel),
                tokens,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let mut summary_lines = column![
        text(
            "The library's .snxlib file is intact, but its .git/ \
             directory is missing. Past per-primitive history can't \
             be read until the repository is restored."
        )
        .size(11)
        .color(muted),
        Space::new().height(8),
        text(format!("Library: {}", path.display()))
            .size(11)
            .color(muted),
    ]
    .spacing(2);

    if let Some(url) = remote {
        summary_lines = summary_lines.push(Space::new().height(4));
        summary_lines = summary_lines.push(text(format!("Remote: {}", url)).size(11).color(muted));
    }

    summary_lines = summary_lines.push(Space::new().height(10));
    summary_lines = summary_lines.push(
        text(
            "Re-init creates a fresh repository at the parent \
             directory. Past commits are lost; new edits are tracked \
             from this point forward.",
        )
        .size(11)
        .color(iced::Color::from_rgb(0.86, 0.40, 0.36)),
    );

    let restore_button: Element<'a, LibraryMessage> = if remote.is_some() {
        secondary_btn(
            "Restore from remote",
            LibraryMessage::RecoveryGitMissing(GitMissingChoice::RestoreFromRemote),
            text_c,
            border,
        )
    } else {
        disabled_btn("Restore from remote", text_c, border)
    };

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            secondary_btn(
                "Cancel",
                LibraryMessage::RecoveryGitMissing(GitMissingChoice::Cancel),
                text_c,
                border,
            ),
            Space::new().width(8),
            secondary_btn(
                "Skip (read-only)",
                LibraryMessage::RecoveryGitMissing(GitMissingChoice::Skip),
                text_c,
                border,
            ),
            Space::new().width(8),
            restore_button,
            Space::new().width(8),
            destructive_btn(
                "Re-init (lose history)",
                LibraryMessage::RecoveryGitMissing(GitMissingChoice::ReInit),
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, container(summary_lines).padding([14, 16]), footer]
            .width(Length::Fixed(MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}

fn broken_binding_view<'a>(
    library_path: &'a std::path::Path,
    table: &'a str,
    row_id: Uuid,
    kind: PrimitiveKind,
    missing_uuid: Uuid,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let kind_label = match kind {
        PrimitiveKind::Symbol => "Symbol",
        PrimitiveKind::Footprint => "Footprint",
        PrimitiveKind::Sim => "Sim Model",
        // PrimitiveKind is #[non_exhaustive]; future variants render
        // generically until the dialog gets a per-kind override.
        _ => "Primitive",
    };

    let header = container(
        row![
            text(format!("Broken {} Binding", kind_label))
                .size(14)
                .color(text_c),
            Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Cancel),
                tokens,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let body = column![
        text(format!(
            "A row in '{}' references a {} that no longer exists on disk.",
            table,
            kind_label.to_lowercase()
        ))
        .size(11)
        .color(muted),
        Space::new().height(8),
        text(format!("Library: {}", library_path.display()))
            .size(11)
            .color(muted),
        text(format!("Row: {}", row_id)).size(11).color(muted),
        text(format!("Missing UUID: {}", missing_uuid))
            .size(11)
            .color(muted),
    ]
    .spacing(2);

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            secondary_btn(
                "Cancel",
                LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Cancel),
                text_c,
                border,
            ),
            Space::new().width(8),
            secondary_btn(
                "Remove row",
                LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::RemoveRow),
                text_c,
                border,
            ),
            Space::new().width(8),
            primary_btn(
                "Re-bind\u{2026}",
                LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Rebind),
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, container(body).padding([14, 16]), footer].width(Length::Fixed(MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}

fn secondary_btn<'a>(
    label: &'a str,
    message: LibraryMessage,
    text_color: iced::Color,
    border: iced::Color,
) -> Element<'a, LibraryMessage> {
    button(container(text(label.to_string()).size(11).color(text_color)).padding([4, 14]))
        .on_press(message)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        })
        .into()
}

fn primary_btn<'a>(label: &'a str, message: LibraryMessage) -> Element<'a, LibraryMessage> {
    let bg = iced::Color::from_rgb(0.00, 0.47, 0.84);
    let fg = iced::Color::WHITE;
    button(container(text(label.to_string()).size(11).color(fg)).padding([4, 14]))
        .on_press(message)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: fg,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .into()
}

/// Destructive button — used for *Re-init (lose history)*. Painted
/// muted-red so the lossy semantics read at a glance. Confirm with
/// the user before tweaking the colour.
fn destructive_btn<'a>(label: &'a str, message: LibraryMessage) -> Element<'a, LibraryMessage> {
    let bg = iced::Color::from_rgb(0.74, 0.25, 0.22);
    let fg = iced::Color::WHITE;
    button(container(text(label.to_string()).size(11).color(fg)).padding([4, 14]))
        .on_press(message)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: fg,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .into()
}

fn disabled_btn<'a>(
    label: &'a str,
    text_color: iced::Color,
    border: iced::Color,
) -> Element<'a, LibraryMessage> {
    let muted = iced::Color {
        a: text_color.a * 0.5,
        ..text_color
    };
    container(container(text(label.to_string()).size(11).color(muted)).padding([4, 14]))
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: iced::Color {
                    a: border.a * 0.5,
                    ..border
                },
            },
            ..Default::default()
        })
        .into()
}

fn close_x<'a>(message: LibraryMessage, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
        .on_press(message)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                )),
                _ => Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.03,
                ))),
            };
            iced::widget::button::Style {
                background: bg,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
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
