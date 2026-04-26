//! "Close Library — Unsaved Drafts" confirmation modal.
//!
//! Mirrors the dock subsystem's `ProjectCloseConfirm` flow (see
//! `app/view/dialogs.rs::view_project_close_confirm_body`) — when
//! the user closes a library that still has at least one editor
//! window with `dirty = true`, the dispatcher diverts to
//! [`crate::library::messages::LibraryMessage::ConfirmCloseLibrary`]
//! and shows this modal listing every dirty draft so the user can
//! Save All / Discard All / Cancel.
//!
//! Reuses the picker modal's chrome (`modal_card` /
//! `modal_header_strip` / `modal_footer_strip`) for visual parity
//! across the Library subsystem.

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{CloseLibraryChoice, LibraryMessage};
use super::state::{CloseLibraryConfirmState, LibraryState};

const MODAL_W: f32 = 520.0;
const MODAL_H: f32 = 380.0;

pub fn view<'a>(
    state: &'a LibraryState,
    confirm: &'a CloseLibraryConfirmState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("Close Library — Unsaved Drafts")
                .size(14)
                .color(text_c),
            Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel),
                tokens,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let summary = text(format!(
        "'{}' has {} unsaved draft(s). What would you like to do?",
        confirm.library_name,
        confirm.dirty_editors.len()
    ))
    .size(11)
    .color(muted);

    // List of dirty drafts by display_internal_pn. We look each one
    // up in `state.open_editors` so the modal stays accurate even
    // if the user opens / closes other editors while it's up.
    let mut rows: Vec<Element<'_, LibraryMessage>> = Vec::new();
    for win_id in &confirm.dirty_editors {
        let label = state
            .open_editors
            .get(win_id)
            .map(|st| st.display_internal_pn.clone())
            .unwrap_or_else(|| format!("(window {win_id:?})"));
        rows.push(
            container(text(label).size(11).color(text_c))
                .padding([4, 8])
                .into(),
        );
    }
    let list = scrollable(column(rows).spacing(0))
        .height(Length::Fixed(140.0))
        .width(Length::Fill);

    let body = column![
        container(summary).padding([14, 16]),
        container(list).padding([0, 16]),
    ]
    .spacing(0);

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            secondary_btn(
                "Cancel",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel),
                text_c,
                border,
            ),
            Space::new().width(8),
            secondary_btn(
                "Discard All",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::DiscardAll),
                text_c,
                border,
            ),
            Space::new().width(8),
            primary_btn(
                "Save All",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::SaveAll),
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, body, Space::new().height(Length::Fill), footer]
            .width(Length::Fixed(MODAL_W))
            .height(Length::Fixed(MODAL_H)),
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
