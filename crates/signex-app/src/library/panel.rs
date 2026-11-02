//! Library left-dock panel — the open libraries, each expandable.
//!
//! The per-library row grid lives in the main-window Library Browser
//! tab (see [`crate::library::browser`]). Here each open library is a
//! row whose ▸ expands it to list its components (#1895): one click
//! highlights a component, a double click opens it. `[Open]` opens the
//! library's browser tab.
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

use iced::widget::{Column, Space, button, column, container, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{LibraryMessage, PickerMsg};
use super::state::LibraryState;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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
        .on_input(|s| LibraryMessage::Picker(PickerMsg::PanelSearchChanged(s)))
        .padding(LIB_PANEL_ROW_PADDING)
        .size(LIB_PANEL_TEXT_SIZE);

    let mut col: Column<'a, LibraryMessage> = column![]
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(container(search).padding([SP.s, SP.s]));

    if state.open_libraries.is_empty() {
        col = col.push(
            container(
                text("No libraries open. Use File ▸ Library ▸ Open Library… to open a *.snxlib/.")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.l, SP.m]),
        );
    } else {
        for lib in state.open_libraries.iter() {
            // Apply the library-name filter when a needle is set.
            if !needle.is_empty() && !lib.display_name.to_lowercase().contains(&needle) {
                continue;
            }
            let expanded = state.panel_expanded.contains(&lib.root);
            col = col.push(container(library_row(lib, expanded, tokens)).padding([SP.xs, SP.s]));
            if expanded {
                col = col.push(component_rows(lib, state, tokens));
            }
        }
    }
    let footer = container(
        row![
            button(
                text("+ Open Library…")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(text_c)
            )
            .padding([SP.s, SP.m])
            .on_press(LibraryMessage::OpenLibraryDialog)
            .style(crate::styles::menu_item(tokens)),
        ]
        .spacing(SP.s),
    )
    .padding([6.0, SP.s]);

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
        crate::panels::panel_scrollable(body).height(Length::Fill),
        Space::new().height(2),
        footer
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// One library row: the ▸ / ▾ toggle, the name and row count, and Open.
fn library_row<'a>(
    lib: &'a super::state::OpenLibrary,
    expanded: bool,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let toggle = button(
        row![
            text(if expanded { "▾" } else { "▸" })
                .size(LIB_PANEL_TEXT_SIZE)
                .color(text_c),
            text(format!(
                "{}.snxlib  ({} rows)",
                lib.display_name,
                lib.total_rows()
            ))
            .size(LIB_PANEL_TEXT_SIZE)
            .color(text_c),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.xs])
    .width(Length::Fill)
    .on_press(LibraryMessage::Picker(PickerMsg::PanelLibraryToggled(
        lib.root.clone(),
    )))
    .style(crate::styles::subtle_button(tokens));
    let open = button(
        text("Open")
            .size(LIB_PANEL_TEXT_SIZE)
            .color(crate::styles::tokens::on_accent(tokens)),
    )
    .padding([SP.xs, SP.m])
    .on_press(LibraryMessage::OpenLibraryBrowser(lib.root.clone()))
    .style(crate::styles::accent_button(tokens));
    row![toggle, open]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .into()
}

/// The components of an expanded library, one indented row each, tables
/// by name and rows in stored order.
fn component_rows<'a>(
    lib: &'a super::state::OpenLibrary,
    state: &'a LibraryState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let mut tables: Vec<&String> = lib.tables.keys().collect();
    tables.sort();
    let mut list: Column<'a, LibraryMessage> = column![].spacing(0).width(Length::Fill);
    let mut any = false;
    for table in tables {
        // Stored order, as the Library Browser shows it: no per-frame sort.
        for r in &lib.tables[table] {
            any = true;
            let key = super::state::ComponentsPanelRowKey {
                library_path: lib.root.clone(),
                table: table.clone(),
                row_id: signex_library::RowId::from_uuid(r.row_id),
            };
            let selected = state.panel_selected.as_ref() == Some(&key);
            list = list.push(
                button(
                    text(r.internal_pn.as_str().to_owned())
                        .size(LIB_PANEL_TEXT_SIZE)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .padding([SP.xs / 2.0, SP.xl * 2.0])
                .width(Length::Fill)
                .on_press(LibraryMessage::Picker(PickerMsg::PanelComponentClicked(
                    key,
                )))
                .style(crate::styles::hover_row(tokens, selected)),
            );
        }
    }
    if !any {
        list = list.push(
            container(
                text("No components in this library.")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.xs, SP.xl * 2.0]),
        );
    }
    list.into()
}
