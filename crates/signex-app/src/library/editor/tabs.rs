//! Tab pill router for the Component Editor — reuses
//! [`signex_widgets::tab_pill::TabPill`] so the editor tabs share
//! the document tab bar's chrome.
//!
//! WS-G: Pin Map — the tab pill list is data-driven from
//! `EditorTab::ORDER`, so adding `EditorTab::PinMap` to the slice in
//! `state.rs` is the only change this module needs. No new pill code
//! lives here.

use iced::widget::{button, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::EditorTab;

pub fn view<'a>(
    active: EditorTab,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let accent = crate::styles::ti(tokens.accent);
    let active_fill = crate::styles::ti(tokens.hover);
    let inactive_fill = iced::Color {
        a: active_fill.a * 0.35,
        ..active_fill
    };

    let mut row_widget = row![].spacing(0).align_y(iced::Alignment::Center);
    let last_idx = EditorTab::ORDER.len() - 1;
    for (i, tab) in EditorTab::ORDER.iter().enumerate() {
        let is_active = *tab == active;
        let style = TabPillStyle {
            fill: if is_active {
                active_fill
            } else {
                inactive_fill
            },
            border,
            accent,
            is_active,
            is_last: i == last_idx,
            accent_position: AccentPosition::Bottom,
        };
        // The content of each pill — clickable button so we can wire
        // `on_press` without writing a custom Widget. The TabPill
        // wraps it for the chrome.
        let label = container(text(tab.label()).size(11).color(text_c)).padding([3, 10]);
        let inner_btn = button(label)
            .padding(0)
            .on_press(LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SelectTab(*tab),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: None,
                text_color: text_c,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            });
        let pill = TabPill::new(inner_btn, style);
        row_widget = row_widget.push(pill);
    }

    container(row_widget)
        .padding([2, 4])
        .width(Length::Fill)
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}
