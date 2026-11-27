//! Components Panel: every mounted part in one table, one category at a
//! time (mockup row 52, components-cx decisions 1 to 9).
//!
//! The tool line picks the category (All parts or a component class), opens
//! the Filter and Columns popovers and switches between the list alone and
//! the list with details; the search field runs under it. The group bar,
//! the table and its status line follow ([`table`]). Each category opens
//! with the columns, sort and grouping of its class template; the user's
//! changes are saved per user and Reset to category default returns to the
//! template's.
//!
//! With the details on, the splitter divides the table from the selected
//! part's details: previews, the footprint pick, every parameter with pins
//! to the top, references and part choices.
//!
//! Search is value-aware (`query`): `100n` finds a 0.1 µF part, a bare
//! `100` every measurement showing 100, and other words are case-insensitive
//! substrings of the part number, MPN, manufacturer, description or library
//! name. An info line under the header counts what each bare number matched.

use iced::widget::{Column, Space, column, container};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_widgets::{button, raw, text};

use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::messages::LibraryMessage;
use crate::library::state::LibraryState;
use crate::styles::Spacing;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: Spacing = Spacing::DEFAULT;

pub mod details;
mod empty;
pub mod global_prefs;
mod look;
pub(crate) mod query;
pub mod recent;
mod rows;
pub mod split;
pub mod stock;
pub mod table;
pub mod thumbs;

const ROW_TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.body;
/// Hints, counts, manufacturer and empty-state subtext; also the layout unit
/// for the pointer-travel and list-height constants.
const HINT_TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.caption;
/// Pointer travel that turns a row press into a drag.
pub(crate) const DRAG_THRESHOLD: f32 = HINT_TEXT_SIZE;
/// The command whose key a row menu shows beside Place and accepts while the
/// menu is open (docs/DECISIONS.md, 2026-11-27, #2631).
pub(crate) const ROW_PLACE_COMMAND: &str = "placement_accept";

/// Render the Components Panel. Returns a `LibraryMessage`-typed
/// element; the dock host wraps it in `DockMessage::Library` to route
/// through the existing library dispatcher.
pub(crate) fn view<'a>(
    state: &'a LibraryState,
    ctx: &'a crate::panels::PanelContext,
    tokens: &'a ThemeTokens,
    grid: details::symbol_canvas::SheetGrid,
) -> Element<'a, LibraryMessage> {
    let panel = &state.components_panel;
    let i18n = &*ctx.i18n;
    // Nothing mounted: the card, and the menu its Add Library… opens.
    if state.open_libraries.is_empty() {
        let mut empty = column![empty::nothing_mounted(tokens, i18n)].spacing(0);
        if panel.add_menu_open {
            empty = empty.push(look::add_menu(tokens, i18n));
        }
        return container(empty)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed();
    }

    // ── Tool line, then the search field across the dock ──────────
    let mut col: Column<Element<'a, LibraryMessage>> = column![
        container(
            column![
                table::toolbar(&panel.table, tokens, i18n),
                look::filter_field(&panel.filter, tokens, i18n),
            ]
            .spacing(0)
        )
        .padding(iced::Padding {
            top: SP.s,
            right: look::EDGE,
            bottom: 0.0,
            left: look::EDGE,
        })
        .width(Length::Fill),
    ]
    .spacing(0)
    .width(Length::Fill);
    if let Some(summary) = look::match_summary(panel.rows.match_summary(), tokens, i18n) {
        col = col.push(summary);
    }
    if !state.id_clash_notices.is_empty() {
        col = col.push(id_clash_notices(state, tokens, i18n));
    }
    col = col.push(thin_sep(look::separator(tokens)));
    let block = table::table_block(state, tokens, i18n);
    if panel.table.list_only {
        return col
            .push(container(block).height(Length::Fill))
            .height(Length::Fill)
            .boxed();
    }
    let divider = split::handle(panel.split_hovered, panel.split_dragging, tokens);
    let library_name = panel
        .selected
        .as_ref()
        .and_then(|key| state.library_at(&key.library_path))
        .map(|lib| lib.display_name.as_str());
    col.push(container(block).height(panel.list_height))
        .push(divider)
        .push(
            container(details::view(
                panel,
                library_name,
                tokens,
                &ctx.canvas_colors,
                grid,
                ctx.device_px_per_logical_px,
                i18n,
            ))
            .height(Length::Fill)
            .width(Length::Fill),
        )
        .height(Length::Fill)
        .boxed()
}

/// The symbol references the panel would draw: the dragged row's card
/// shows one.
pub(crate) fn wanted_thumbnails(
    state: &LibraryState,
) -> Vec<signex_library::primitive::PrimitiveRef> {
    let panel = &state.components_panel;
    panel
        .drag
        .as_ref()
        .and_then(|drag| recent::row_of(state, &drag.key))
        .map(|row| row.symbol_ref)
        .into_iter()
        .collect()
}

/// Non-modal list of libraries refused or closed for sharing a library id
/// with a stronger source (#1908), with one Dismiss button.
fn id_clash_notices<'a>(
    state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let mut notices: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.xs);
    for notice in &state.id_clash_notices {
        notices = notices.push(
            // raw: the id-clash notice is worded in English by the library
            // mount (#2060).
            text(raw(notice.message()))
                .size(ROW_TEXT_SIZE)
                .color(text_c)
                .boxed(),
        );
    }
    let dismiss = button(
        text(t!(i18n, ids::PANELS_COMPONENTS_DISMISS))
            .size(ROW_TEXT_SIZE)
            .color(text_c),
    )
    .padding([SP.xs, SP.m])
    .on_press(LibraryMessage::IdClashNoticesDismissed)
    .style(crate::styles::menu_item(tokens));
    container(notices.push(dismiss.boxed()))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

fn thin_sep<'a, M: 'a>(border_c: Color) -> Element<'a, M> {
    container(Space::new())
        .height(1.0)
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(border_c)),
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "rows_tests.rs"]
mod rows_tests;
