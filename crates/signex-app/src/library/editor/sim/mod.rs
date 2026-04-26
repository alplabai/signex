//! Sim tab — SPICE model body + pin-to-node binding (LIBRARY_PLAN §10).
//!
//! Layout:
//!
//! 1. Header row with the "Has SPICE model" toggle.
//! 2. Multi-line `text_editor` for the SPICE body (monospace).
//! 3. Two-column grid mapping Standard pin numbers → SPICE node names.
//!
//! When the toggle is off the body / pin grid are hidden and the
//! `draft.shared.simulation` field is forced to `None`. Pin numbers
//! come from the parent component's symbol body via
//! [`state::extract_pin_numbers`]; an empty / unparseable body falls
//! back to a numeric `1..N` skeleton driven by an editable count.

pub mod state;

use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;
use crate::fonts::DEFAULT_CANVAS_FONT;
pub use state::{SimTabState, apply_pin_node_edit, extract_pin_numbers, seed_empty_pin_map};

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let enabled = editor.draft.shared.simulation.is_some();

    let header = row![
        checkbox(enabled)
            .label("Has SPICE model")
            .on_toggle(move |on| LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SimSetEnabled(on),
            }),
        Space::new().width(Length::Fill),
        text(if enabled {
            format!(
                "{} pin{}",
                editor.sim.pin_numbers.len(),
                if editor.sim.pin_numbers.len() == 1 {
                    ""
                } else {
                    "s"
                }
            )
        } else {
            "—".to_string()
        })
        .size(11)
        .color(muted),
    ]
    .padding([0, 4])
    .align_y(iced::Alignment::Center);

    let mut col = column![header].spacing(0).width(Length::Fill);

    if enabled {
        col = col.push(Space::new().height(10));
        col = col.push(view_body_card(editor, tokens, window_id));
        col = col.push(Space::new().height(10));
        col = col.push(view_pin_map_card(editor, tokens, window_id));
    } else {
        col = col.push(Space::new().height(10));
        col = col.push(disabled_hint_card(tokens));
    }

    container(col)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn disabled_hint_card<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    container(
        text(
            "Toggle Has SPICE model on to author a body and pin-to-node \
             binding. While off the saved component carries no simulation \
             metadata.",
        )
        .size(11)
        .color(muted),
    )
    .padding(14)
    .width(Length::Fill)
    .style(crate::styles::modal_card(tokens))
    .into()
}

fn view_body_card<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let body_editor = iced::widget::text_editor(&editor.sim.body)
        .on_action(move |action| LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SimBodyAction(action),
        })
        .font(iced::Font::with_name(DEFAULT_CANVAS_FONT))
        .size(12)
        .height(Length::Fixed(220.0))
        .padding(8);

    container(
        column![
            text("SPICE body").size(11).color(text_c),
            Space::new().height(2),
            text("Multi-line monospace — paste a SUBCKT, .MODEL, or behavioural source.")
                .size(10)
                .color(muted),
            Space::new().height(8),
            body_editor,
        ]
        .spacing(0)
        .width(Length::Fill),
    )
    .padding(14)
    .width(Length::Fill)
    .style(crate::styles::modal_card(tokens))
    .into()
}

fn view_pin_map_card<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = row![
        text("Pin").size(10).color(muted).width(Length::Fixed(80.0)),
        Space::new().width(8),
        text("SPICE node").size(10).color(muted).width(Length::Fill),
    ]
    .padding([2, 8]);

    let mut rows = column![header].spacing(2);

    let pin_map_owned = editor
        .draft
        .shared
        .simulation
        .as_ref()
        .map(|m| m.pin_map.clone())
        .unwrap_or_default();

    let pin_numbers = if editor.sim.pin_numbers.is_empty() {
        // No symbol pins → use BTreeMap keys as the row driver if the
        // user already populated some, otherwise fall back to a single
        // "1" row so the grid is never blank.
        if pin_map_owned.is_empty() {
            vec!["1".to_string()]
        } else {
            pin_map_owned.keys().cloned().collect()
        }
    } else {
        editor.sim.pin_numbers.clone()
    };

    for pin_number in pin_numbers.iter().cloned() {
        let value = pin_map_owned.get(&pin_number).cloned().unwrap_or_default();
        let pin_for_msg = pin_number.clone();
        let pin_input = text_input("node name", &value)
            .padding([4, 8])
            .size(11)
            .on_input(move |s| LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SimSetPinNode {
                    pin_number: pin_for_msg.clone(),
                    value: s,
                },
            });
        let row_widget = row![
            container(text(pin_number.clone()).size(11).color(text_c))
                .padding([4, 8])
                .width(Length::Fixed(80.0))
                .style(move |_: &Theme| iced::widget::container::Style {
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..iced::widget::container::Style::default()
                }),
            Space::new().width(8),
            container(pin_input).width(Length::Fill),
        ]
        .padding([0, 8])
        .align_y(iced::Alignment::Center);
        rows = rows.push(row_widget);
    }

    let mut footer_row = row![].spacing(8).align_y(iced::Alignment::Center);
    if editor.sim.pin_numbers.is_empty() {
        footer_row = footer_row.push(
            text(
                "Symbol body has no parseable pins yet — fill in the placeholder \
                 row or edit the Symbol tab to add pins.",
            )
            .size(10)
            .color(muted),
        );
    }
    footer_row = footer_row.push(Space::new().width(Length::Fill));
    footer_row = footer_row.push(
        button(container(text("Reset to skeleton").size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SimChanged(signex_library::SpiceModel {
                    body: editor.sim.body_text(),
                    pin_map: seed_empty_pin_map(if editor.sim.pin_numbers.is_empty() {
                        &[]
                    } else {
                        editor.sim.pin_numbers.as_slice()
                    }),
                }),
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
            }),
    );

    container(
        column![
            text("Pin → SPICE node").size(11).color(text_c),
            Space::new().height(2),
            text(
                "Each Standard pin number maps to a SPICE node label. Empty rows are pruned on save."
            )
            .size(10)
            .color(muted),
            Space::new().height(8),
            scrollable(rows)
                .height(Length::Fixed(200.0))
                .width(Length::Fill),
            Space::new().height(6),
            footer_row,
        ]
        .spacing(0)
        .width(Length::Fill),
    )
    .padding(14)
    .width(Length::Fill)
    .style(crate::styles::modal_card(tokens))
    .into()
}
