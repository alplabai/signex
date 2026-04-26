//! Params tab — editable `(key, value)` grid backed by
//! `SharedSide.parameters`.
//!
//! Phase 1 only edits `ParamValue::Text`. Numeric / measurement
//! variants land in Phase 2 with a kind picker per row.

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::ParamValue;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = row![
        text("Key")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        Space::new().width(8),
        text("Value (text)")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
        Space::new().width(8),
        text("").size(10).width(Length::Fixed(60.0)),
    ]
    .padding([2, 8]);

    let mut rows = column![header].spacing(2);
    let mut idx = 0usize;
    for (key, value) in editor.draft.shared.parameters.iter() {
        let value_str = match value {
            ParamValue::Text(s) => s.clone(),
            ParamValue::Number(n) => n.to_string(),
            ParamValue::Bool(b) => b.to_string(),
            ParamValue::Measurement { value, unit } => format!("{value} {unit}"),
        };
        let row_idx = idx;
        let key_input = text_input("key", key.as_str())
            .padding([4, 8])
            .size(11)
            .on_input(move |s| LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::ParamSetKey {
                    idx: row_idx,
                    key: s,
                },
            });
        let val_input = text_input("value", &value_str)
            .padding([4, 8])
            .size(11)
            .on_input(move |s| LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::ParamSetValueText {
                    idx: row_idx,
                    value: s,
                },
            });
        let remove_btn =
            button(container(text("\u{2212} Remove").size(10).color(text_c)).padding([3, 8]))
                .on_press(LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::ParamRemoveRow(row_idx),
                })
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
        let r = row![
            container(key_input).width(Length::FillPortion(2)),
            Space::new().width(8),
            container(val_input).width(Length::FillPortion(3)),
            Space::new().width(8),
            container(remove_btn).width(Length::Fixed(80.0)),
        ]
        .padding([0, 8])
        .align_y(iced::Alignment::Center);
        rows = rows.push(r);
        idx += 1;
    }

    if idx == 0 {
        rows = rows.push(
            container(
                text("No parameters yet. Click + Add Parameter to create the first row.")
                    .size(11)
                    .color(muted),
            )
            .padding([8, 8]),
        );
    }

    let add_btn =
        button(container(text("+ Add Parameter").size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::ParamAddRow,
            })
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

    container(
        column![
            scrollable(rows).height(Length::Fill).width(Length::Fill),
            Space::new().height(8),
            row![add_btn],
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .style(crate::styles::modal_card(tokens))
    .padding(14)
    .into()
}
