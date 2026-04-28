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
//! `LibraryState::create_options.is_some()`. The matching
//! `needs_overlay` predicate must include the same flag (memory
//! note `[needs_overlay predicate gates modal rendering]`).

use iced::widget::{Space, button, checkbox, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::LibraryCreateOptionsState;

const MODAL_W: f32 = 480.0;

pub fn view<'a>(
    state: &'a LibraryCreateOptionsState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let lib_label = state
        .lib_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| state.lib_path.display().to_string());

    let header = container(
        row![
            text("Library Options").size(14).color(text_c),
            Space::new().width(Length::Fill),
            text(lib_label).size(11).color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    // Single toggle row: checkbox + label/description stack.
    let lfs_check: Element<'a, LibraryMessage> = checkbox(state.use_lfs)
        .size(14)
        .on_toggle(|_| LibraryMessage::LibraryCreateOptionsToggleLfs)
        .into();

    let lfs_label = text("Use Git LFS for binary 3D models")
        .size(12)
        .color(text_c);
    let lfs_help = text(
        "Tracks `*.step`, `*.stp`, `*.wrl`, `*.iges` via Git LFS \
         (recommended for production libraries). Requires `git lfs` \
         to be installed locally.",
    )
    .size(10)
    .color(muted);

    let lfs_row = container(
        row![
            lfs_check,
            Space::new().width(8),
            column![lfs_label, lfs_help].spacing(4),
        ]
        .align_y(iced::Alignment::Start),
    )
    .padding([10, 14])
    .width(Length::Fill);

    let body = column![lfs_row].spacing(2);

    let cancel = button(text("Cancel").size(11).color(text_c))
        .padding([4, 14])
        .on_press(LibraryMessage::LibraryCreateOptionsCancel)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });
    let create = button(text("Create Library").size(11).color(iced::Color::WHITE))
        .padding([4, 14])
        .on_press(LibraryMessage::LibraryCreateOptionsConfirm)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.00, 0.47, 0.84,
            ))),
            text_color: iced::Color::WHITE,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            cancel,
            Space::new().width(8),
            create
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, container(body).padding([6, 0]), footer].width(Length::Fixed(MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}
