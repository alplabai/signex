//! Library left-dock panel — the open libraries, each expandable.
//!
//! The per-library row grid lives in the main-window Library Browser
//! tab (see [`crate::library::browser`]). Here each open library is a
//! row whose chevron expands it to list its tables and, under each, its
//! components (#1895): one click highlights a component, a double click opens it. `[Open]` opens the
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

use iced::widget::{Column, Space, Widget, column, container, row};
use iced::{Border, Element, Length, Theme};
use signex_i18n::{ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, raw, text, text_input};

use super::messages::{LibraryMessage, PickerMsg};
use super::state::LibraryState;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const LIB_PANEL_TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.body;
const LIB_PANEL_ROW_PADDING: u16 = 4;

/// Render the Library left-dock panel.
pub fn view<'a>(
    state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let needle = i18n.fold_for_search(state.panel_search.trim());

    // The search box filters library *names*, not rows. Component-row
    // search lives inside the Library Browser tab (each browser tab
    // has its own search buffer).
    let search = text_input(t!(i18n, ids::LIBRARY_PANEL_SEARCH), &state.panel_search)
        .on_input(|s| LibraryMessage::Picker(PickerMsg::PanelSearchChanged(s)))
        .padding(LIB_PANEL_ROW_PADDING)
        .size(LIB_PANEL_TEXT_SIZE)
        .style(crate::panels::form_look::text_input_style);

    let mut col: Column<Element<'a, LibraryMessage>> = column![]
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(container(search).padding([SP.s, SP.s]).boxed());

    if state.open_libraries.is_empty() {
        col = col.push(
            container(
                text(t!(i18n, ids::LIBRARY_PANEL_NONE_OPEN))
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.l, SP.m])
            .boxed(),
        );
    } else {
        for lib in state.open_libraries.iter() {
            // Apply the library-name filter when a needle is set.
            if !needle.is_empty() && !i18n.fold_for_search(&lib.display_name).contains(&needle) {
                continue;
            }
            let expanded = state.panel_expanded.contains(&lib.root);
            col = col.push(
                container(library_row(lib, expanded, tokens, i18n))
                    .padding([SP.xs, SP.s])
                    .boxed(),
            );
            if expanded {
                col = col.push(component_rows(lib, state, tokens, i18n));
            }
        }
    }
    let footer = container(
        row![
            button(
                text(t!(i18n, ids::LIBRARY_PANEL_OPEN_LIBRARY))
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(text_c)
            )
            .padding([SP.s, SP.m])
            .on_press(LibraryMessage::OpenLibraryDialog)
            .style(crate::styles::menu_item(tokens)),
        ]
        .spacing(SP.s),
    )
    .padding([6.0, SP.s])
    .boxed();

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
        crate::panels::panel_scrollable(body.boxed()).height(Length::Fill),
        Space::new().height(2),
        footer
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// One library row: the chevron toggle, the name and row-count pill, and Open.
fn library_row<'a>(
    lib: &'a super::state::OpenLibrary,
    expanded: bool,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let chevron = signex_widgets::chevron::chevron(
        expanded,
        LIB_PANEL_TEXT_SIZE,
        crate::styles::tokens::resolve(tokens).ink3,
    );
    let rows_pill = crate::library::editor::kit::toned_pill(
        t!(i18n, ids::LIBRARY_PANEL_ROWS, "count" => lib.total_rows()).into_string(),
        crate::styles::PillTone::Neutral,
        tokens,
    );
    let toggle = button(
        row![
            chevron,
            text(signex_i18n::UserText::new(format!(
                "{}.snxlib",
                lib.display_name
            )))
            .size(LIB_PANEL_TEXT_SIZE)
            .color(text_c),
            rows_pill,
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
        text(signex_i18n::t!(i18n, signex_i18n::ids::LIBRARY_PANEL_OPEN))
            .size(LIB_PANEL_TEXT_SIZE)
            .color(text_c),
    )
    .padding([SP.xs, SP.m])
    .on_press(LibraryMessage::OpenLibraryBrowser(lib.root.clone()))
    .style(crate::panels::form_look::ghost_button);
    row![toggle, open]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// The tree of an expanded library: each table as an indented header
/// with its components one level further in, tables by name and rows in
/// stored order.
fn component_rows<'a>(
    lib: &'a super::state::OpenLibrary,
    state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let tables = lib.panel_tables();
    let mut list: Column<Element<'a, LibraryMessage>> = column![].spacing(0).width(Length::Fill);
    if tables.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::LIBRARY_PANEL_NO_TABLES))
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.xs, SP.xl])
            .boxed(),
        );
    }
    for (table, rows) in tables {
        list = list.push(
            container(
                text(raw(format!("{table}  ({})", rows.len())))
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.xs, SP.xl])
            .boxed(),
        );
        for r in rows {
            let key = super::state::ComponentsPanelRowKey {
                library_path: lib.root.clone(),
                table: table.to_owned(),
                row_id: signex_library::RowId::from_uuid(r.row_id),
            };
            let selected = state.panel_selected.as_ref() == Some(&key);
            list = list.push(
                button(
                    text(signex_i18n::UserText::new(
                        r.internal_pn.as_str().to_owned(),
                    ))
                    .size(LIB_PANEL_TEXT_SIZE)
                    .wrapping(iced::widget::text::Wrapping::None),
                )
                .padding([SP.xs / 2.0, SP.xl * 2.0])
                .width(Length::Fill)
                .on_press(LibraryMessage::Picker(PickerMsg::PanelComponentClicked(
                    key,
                )))
                .style(crate::styles::hover_row(tokens, selected))
                .boxed(),
            );
        }
    }
    list.boxed()
}
