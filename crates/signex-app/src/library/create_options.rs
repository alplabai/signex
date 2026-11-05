//! "Library Options" modal — Stage 11 of
//! `v0.9-snxlib-as-file-plan.md`.
//!
//! Pops up between the New Library Save-As dialog (where the user
//! picked the `.snxlib` filename) and the actual disk + git init
//! call. Carries a single user-facing toggle: "Use Git LFS for
//! binary 3D models". Defaults off so `git lfs` doesn't become a
//! hard prerequisite for casual library creation; production
//! libraries flip it on at create time so 3D model commits don't
//! bloat the git pack.
//!
//! Mounted as a full-screen overlay backdrop via
//! `app/view/mod.rs::collect_overlays` when
//! `LibraryState::create_options.is_some()`; `create_options_overlay`
//! owns that guard.

use iced::widget::{Space, checkbox, column, container, row, text};
use iced::{Element, Length, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::modal_chrome::{footer_strip, header_with_note, primary_btn, secondary_btn};
use super::state::LibraryCreateOptionsState;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub fn view<'a>(
    state: &'a LibraryCreateOptionsState,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let lib_label = state
        .lib_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| state.lib_path.display().to_string());

    let header = header_with_note("Library Options", lib_label, windowed, tokens);

    // Toggle 1 — Enable Version Control. Off by default so a fresh
    // library lives on disk as plain files; opting in runs `git init`
    // at the parent directory and stages an initial commit.
    let git_check: Element<'a, LibraryMessage> = checkbox(state.enable_git)
        .size(TS.heading)
        .on_toggle(|_| LibraryMessage::LibraryCreateOptionsToggleGit)
        .boxed();

    let git_label = text("Enable version control (Git)")
        .size(TS.label)
        .color(text_c);
    let git_help = text(
        "Initialise a Git repository at the library's parent directory and \
         commit on every save. Off by default — leave plain files unless \
         you plan to track history or collaborate. Can be enabled later.",
    )
    .size(TS.caption)
    .color(muted);

    let git_row = container(
        row![
            git_check,
            Space::new().width(8),
            column![git_label, git_help].spacing(SP.s),
        ]
        .align_y(iced::Alignment::Start),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill);

    // Toggle 2 — LFS, only meaningful when version control is on.
    // Greyed out otherwise (no `on_toggle` so the checkbox is inert).
    let lfs_check: Element<'a, LibraryMessage> = if state.enable_git {
        checkbox(state.use_lfs)
            .size(TS.heading)
            .on_toggle(|_| LibraryMessage::LibraryCreateOptionsToggleLfs)
            .boxed()
    } else {
        // Inert checkbox — same footprint, can't be clicked.
        checkbox(false).size(TS.heading).boxed()
    };

    let lfs_text_color = if state.enable_git { text_c } else { muted };
    let lfs_label = text("Use Git LFS for binary 3D models")
        .size(TS.label)
        .color(lfs_text_color);
    let lfs_help = text(
        "Tracks `*.step`, `*.stp`, `*.wrl`, `*.iges` via Git LFS \
         (recommended for production libraries). Requires `git lfs` \
         to be installed locally. Only available with version control on.",
    )
    .size(TS.caption)
    .color(muted);

    let lfs_row = container(
        row![
            lfs_check,
            Space::new().width(8),
            column![lfs_label, lfs_help].spacing(SP.s),
        ]
        .align_y(iced::Alignment::Start),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill);

    let mut body = column![git_row, lfs_row].spacing(SP.xs);
    if let Some(reason) = &state.refusal {
        body = body.push(
            container(
                text(reason.as_str())
                    .size(TS.caption)
                    .color(theme_ext::error_color(tokens)),
            )
            .padding([SP.s, SP.l])
            .width(Length::Fill)
            .boxed(),
        );
    }

    let footer = footer_strip(
        Vec::new(),
        vec![
            secondary_btn("Cancel", LibraryMessage::LibraryCreateOptionsCancel, tokens),
            primary_btn(
                "Create Library",
                LibraryMessage::LibraryCreateOptionsConfirm,
                tokens,
            ),
        ],
        tokens,
    );

    container(
        column![header, container(body).padding([SP.s, 0.0]), footer].width(super::card_len(
            windowed,
            super::modal_chrome::CREATE_OPTIONS_W,
        )),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
