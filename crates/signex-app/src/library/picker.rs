//! Component picker modal.
//!
//! Opened from File ▸ Library ▸ Place Component… (and, eventually,
//! the `P` shortcut once Phase 2 wires the placement flow).
//!
//! Shape:
//!
//! ```text
//! ┌─[Place Component ─────────────────────────────────── X]─┐
//! │ [Search internal_pn / mpn / description…]              │
//! ├────────────────────────────────────────────────────────┤
//! │ ► R0805_10k    1.2  Released   Yageo  RC0805FR-…      │
//! │   C0805_100n   1.0  Released   Murata GRM21BR…        │
//! │   …                                                    │
//! ├────────────────────────────────────────────────────────┤
//! │                                  [ Cancel ] [ Place ]  │
//! └────────────────────────────────────────────────────────┘
//! ```

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::commands::list_components_filtered;
use super::messages::{LibraryMessage, PickerMsg};
use super::state::{LibraryState, PickerState};

const PICKER_W: f32 = 720.0;
const PICKER_H: f32 = 480.0;

pub fn view<'a>(
    state: &'a LibraryState,
    picker: &'a PickerState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("Place Component").size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(LibraryMessage::ClosePicker, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let search = text_input("Search internal PN / MPN / description…", &picker.filter)
        .on_input(|s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)))
        .padding(6)
        .size(12);

    let rows = list_components_filtered(state, &picker.filter);

    let mut list_col = column![].spacing(0);
    if rows.is_empty() {
        list_col = list_col.push(
            container(
                text("No components match. Open a library or refine the filter.")
                    .size(11)
                    .color(muted),
            )
            .padding([14, 14]),
        );
    } else {
        for (path, summary) in rows.iter() {
            let is_selected = picker
                .selected
                .as_ref()
                .map(|(p, c)| p == path && c.row_id == summary.row_id)
                .unwrap_or(false);
            let row_bg = if is_selected {
                Some(crate::styles::ti(tokens.hover))
            } else {
                None
            };
            let summary_clone = summary.clone();
            let row_widget = button(
                row![
                    text(summary.internal_pn.as_str().to_string())
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(2)),
                    text(format!("{:?}", summary.state))
                        .size(11)
                        .color(muted)
                        .width(Length::Fixed(96.0)),
                    text(summary.mpn.clone())
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(3)),
                    text(summary.description.clone())
                        .size(11)
                        .color(muted)
                        .width(Length::FillPortion(4)),
                ]
                .spacing(8)
                .padding([3, 8]),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(LibraryMessage::Picker(PickerMsg::SelectComponent(
                summary_clone,
            )))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let hover_bg = crate::styles::ti(tokens.hover);
                let bg = match (row_bg, status) {
                    (Some(bg), _) => Some(iced::Background::Color(bg)),
                    (None, iced::widget::button::Status::Hovered) => {
                        Some(iced::Background::Color(hover_bg))
                    }
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            });
            list_col = list_col.push(row_widget);
        }
    }

    let body = container(
        scrollable(list_col)
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .padding([6, 0])
    .height(Length::Fill);

    let place_enabled = picker.selected.is_some();
    let place_bg = if place_enabled {
        iced::Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let place_fg = if place_enabled {
        iced::Color::WHITE
    } else {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    };
    let mut place_btn = button(container(text("Place").size(11).color(place_fg)).padding([4, 14]))
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(place_bg)),
            text_color: place_fg,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });
    if place_enabled {
        place_btn = place_btn.on_press(LibraryMessage::Picker(PickerMsg::PlaceSelected));
    }

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                .on_press(LibraryMessage::ClosePicker)
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04
                    ))),
                    text_color: text_c,
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..iced::widget::button::Style::default()
                }),
            Space::new().width(8),
            place_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    let body_pad = container(column![container(search).padding([6, 12]), body].spacing(0))
        .width(Length::Fill)
        .height(Length::Fill);

    container(
        column![header, body_pad, footer]
            .width(Length::Fixed(PICKER_W))
            .height(Length::Fixed(PICKER_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
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
