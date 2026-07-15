//! Tools ▸ Document Options modal — Altium SchLib parity.
//!
//! Per-`.snxlib` view settings: sheet color, grid spacing, grid
//! visibility, coordinate display unit. Edits go to a working
//! [`crate::library::state::DocumentOptionsModalState::draft`];
//! Save commits to `OpenLibrary.display`; Cancel discards.
//!
//! Mounted as a full-screen overlay backdrop via
//! `app/view/mod.rs::collect_overlays` when
//! `LibraryState::document_options.is_some()`.

use iced::widget::{Space, button, column, container, pick_list, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::DocumentOptionsModalState;
use crate::panels::SheetColor;

const MODAL_W: f32 = 460.0;

pub fn view<'a>(
    state: &'a DocumentOptionsModalState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("Document Options").size(14).color(text_c),
            Space::new().width(Length::Fill),
            text(format!("Library: {}", state.library_name))
                .size(11)
                .color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let row_field =
        |label: &'a str, value: Element<'a, LibraryMessage>| -> Element<'a, LibraryMessage> {
            container(
                row![
                    text(label)
                        .size(11)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    container(value).width(Length::FillPortion(3)),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([4, 14])
            .width(Length::Fill)
            .into()
        };

    let sheet_picker: Element<'a, LibraryMessage> = pick_list(
        SheetColor::ALL.to_vec(),
        Some(state.draft.sheet_color),
        LibraryMessage::DocumentOptionsSetSheetColor,
    )
    .padding([2, 6])
    .text_size(11)
    .into();

    let pin_selection_picker: Element<'a, LibraryMessage> = pick_list(
        crate::library::state::PinSelectionMode::ALL.to_vec(),
        Some(state.draft.pin_selection),
        LibraryMessage::DocumentOptionsSetPinSelection,
    )
    .padding([2, 6])
    .text_size(11)
    .into();

    let grid_check: Element<'a, LibraryMessage> = iced::widget::checkbox(state.draft.grid_visible)
        .size(14)
        .on_toggle(|_| LibraryMessage::DocumentOptionsToggleGrid)
        .into();

    let grid_label = format!("{:.3} mm", state.draft.grid_size_mm);
    let grid_size_btn: Element<'a, LibraryMessage> =
        button(text(grid_label).size(11).color(text_c))
            .padding([2, 8])
            .on_press(LibraryMessage::DocumentOptionsCycleGridSize)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                text_color: text_c,
                ..iced::widget::button::Style::default()
            })
            .into();

    let unit_label = format!("{}", state.draft.unit);
    let unit_btn: Element<'a, LibraryMessage> = button(text(unit_label).size(11).color(text_c))
        .padding([2, 8])
        .on_press(LibraryMessage::DocumentOptionsCycleUnit)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            text_color: text_c,
            ..iced::widget::button::Style::default()
        })
        .into();

    let body = column![
        row_field("Sheet Color", sheet_picker),
        row_field("Grid Visible", grid_check),
        row_field("Grid Spacing", grid_size_btn),
        row_field("Unit", unit_btn),
        row_field("Pin Selection", pin_selection_picker),
    ]
    .spacing(2);

    let cancel = button(text("Cancel").size(11).color(text_c))
        .padding([4, 14])
        .on_press(LibraryMessage::DocumentOptionsCancel)
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
    let apply = button(text("Apply").size(11).color(iced::Color::WHITE))
        .padding([4, 14])
        .on_press(LibraryMessage::DocumentOptionsApply)
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
            apply
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
