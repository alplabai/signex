//! Library left-dock panel — flat library list (post-WS-K refactor).
//!
//! The inline category-tree-with-row-grid the v0.9-refactor-2 plan §10
//! sketched is now superseded by the main-window Library Browser tab
//! (see [`crate::library::browser`]). This panel is reduced to a flat
//! list of open libraries, each with a single `[Open]` button that
//! opens the browser tab for that library.
//!
//! Shape:
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │ [Search libraries…]                      │
//! ├──────────────────────────────────────────┤
//! │ ▸ Loratis-SN-lib-2.snxlib    [Open]      │
//! │ ▸ Loratis-SN-lib-3.snxlib    [Open]      │
//! │ ─────────────────────────────────────────  │
//! │ [+ Open Library…]                        │
//! └──────────────────────────────────────────┘
//! ```
//!
//! Per-row click on `[Open]` fires `LibraryMessage::OpenLibraryBrowser`
//! which routes through the same handler the project-tree double-click
//! uses.

use iced::widget::{Column, Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{LibraryMessage, PickerMsg};
use super::state::LibraryState;

const LIB_PANEL_TEXT_SIZE: f32 = 11.0;
const LIB_PANEL_ROW_PADDING: u16 = 4;

/// Render the Library left-dock panel.
pub fn view<'a>(state: &'a LibraryState, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let needle = state.panel_search.trim().to_lowercase();

    // The search box filters library *names*, not rows. Component-row
    // search lives inside the Library Browser tab (each browser tab
    // has its own search buffer).
    let search = text_input("Search libraries…", &state.panel_search)
        .on_input(|s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)))
        .padding(LIB_PANEL_ROW_PADDING)
        .size(LIB_PANEL_TEXT_SIZE);

    let mut col: Column<'a, LibraryMessage> = column![]
        .spacing(2)
        .width(Length::Fill)
        .push(container(search).padding([4, 4]));

    if state.open_libraries.is_empty() {
        col = col.push(
            container(
                text("No libraries open. Use File ▸ Library ▸ Open Library… to open a *.snxlib/.")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([12, 8]),
        );
    } else {
        for lib in state.open_libraries.iter() {
            // Apply the library-name filter when a needle is set.
            if !needle.is_empty() && !lib.display_name.to_lowercase().contains(&needle) {
                continue;
            }
            let display_label = format!(
                "▸  {}.snxlib  ({} rows)",
                lib.display_name,
                lib.total_rows()
            );
            let library_path = lib.root.clone();
            let open_msg = LibraryMessage::OpenLibraryBrowser(library_path);
            let label = row![
                text(display_label)
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(text_c)
                    .width(Length::Fill),
                button(
                    text("Open")
                        .size(LIB_PANEL_TEXT_SIZE)
                        .color(iced::Color::WHITE),
                )
                .padding([2, 10])
                .on_press(open_msg)
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb(
                        0.00, 0.47, 0.84,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: Border {
                        width: 0.0,
                        radius: 3.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);
            col = col.push(container(label).padding([2, 6]));
        }
    }

    let footer = container(
        row![
            button(
                text("+ Open Library…")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(text_c)
            )
            .padding([4, 8])
            .on_press(LibraryMessage::OpenLibraryDialog)
            .style(crate::styles::menu_item(tokens)),
        ]
        .spacing(4),
    )
    .padding([6, 4]);

    let body = container(col).style(move |_: &Theme| iced::widget::container::Style {
        background: None,
        border: Border {
            color: border,
            width: 0.0,
            ..Border::default()
        },
        ..Default::default()
    });

    column![
        scrollable(body).width(Length::Fill).height(Length::Fill),
        Space::new().height(2),
        footer
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
