//! The table's toolbar, the table itself and its status line. Pure: it
//! reads the model and the cache `update` built.

use iced::widget::{Column, column, container, pick_list, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, text};

use super::super::thumbs::ThumbLook;
use super::super::{HINT_TEXT_SIZE, ROW_TEXT_SIZE, SP};
use super::columns::caption;
use super::state::{ComponentsTable, TablePopover};
use super::{body, group_bar, header, popovers};
use crate::library::messages::{ComponentsTableMsg, LibraryMessage};
use crate::library::state::{ComponentsPanelState, LibraryState};
use crate::styles::ti;
use crate::styles::tokens::resolve;

fn msg(m: ComponentsTableMsg) -> LibraryMessage {
    LibraryMessage::ComponentsTable(m)
}

/// The tool line: category, Filter (with its count), Columns, the view
/// switch (list only, or list with details) and Previews (the thumbnail
/// column, #2535).
pub(crate) fn toolbar<'a>(
    panel: &'a ComponentsPanelState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let table = &panel.table;
    let entries = table.cache.categories.clone();
    let current = entries.iter().find(|e| e.key == table.category).cloned();
    let category = pick_list(current, entries, |e: &super::category::CategoryEntry| {
        e.label.clone()
    })
    .graphite()
    .on_select(|e: super::category::CategoryEntry| msg(ComponentsTableMsg::CategoryPicked(e.key)))
    .padding([3, 6])
    .width(Length::Fill);
    let active = table.filters().map_or(0, |f| f.active_count());
    let filter_label = if active > 0 {
        t!(i18n, ids::PANELS_COMPONENTS_FILTER_ACTIVE, "count" => active)
    } else {
        t!(i18n, ids::PANELS_COMPONENTS_FILTER_BUTTON)
    };
    let on = |which: TablePopover| table.popover.as_ref() == Some(&which);
    let filter = tool_button(
        filter_label,
        on(TablePopover::Filter) || active > 0,
        ComponentsTableMsg::FilterToggled,
        tokens,
    );
    let columns = tool_button(
        t!(i18n, ids::PANELS_COMPONENTS_COLUMNS_BUTTON),
        on(TablePopover::Columns),
        ComponentsTableMsg::ColumnsToggled,
        tokens,
    );
    let list_only = tool_button(
        t!(i18n, ids::PANELS_COMPONENTS_VIEW_LIST),
        table.list_only,
        ComponentsTableMsg::ListOnlyPicked(true),
        tokens,
    );
    let with_details = tool_button(
        t!(i18n, ids::PANELS_COMPONENTS_VIEW_DETAILS),
        !table.list_only,
        ComponentsTableMsg::ListOnlyPicked(false),
        tokens,
    );
    let previews = super::super::look::previews_switch(panel.row_style, tokens, i18n);
    row![category, filter, columns, list_only, with_details, previews]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .boxed()
}

/// A compact toolbar button; `on` draws it in the accent wash.
fn tool_button<'a>(
    label: signex_i18n::Localized,
    on: bool,
    pressed: ComponentsTableMsg,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (fill, ink) = if on {
        (Some(r.accent_soft), r.accent_soft_ink)
    } else {
        (None, ti(tokens.text))
    };
    let hover = ti(tokens.hover);
    button(
        text(label)
            .size(HINT_TEXT_SIZE)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([3.0, 6.0])
    .on_press(msg(pressed))
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: fill
            .or_else(|| matches!(status, button::Status::Hovered).then_some(hover))
            .map(Background::Color),
        text_color: ink,
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// Everything under the search field: the open popover, the group bar,
/// the table and the status line.
pub(crate) fn table_block<'a>(
    state: &'a LibraryState,
    thumb_look: ThumbLook,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let table = &state.components_panel.table;
    let previews = state.components_panel.row_style.shows_previews();
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(0).width(Length::Fill);
    // One slot either way, so opening a popover keeps the table's widget
    // state (its scroll offset).
    col = col.push(match &table.popover {
        Some(popover) => popovers::view(table, popover, tokens, i18n),
        None => iced::widget::Space::new().height(0.0).boxed(),
    });
    // Also one slot either way, for the same reason.
    col = col.push(stock_notice(state, tokens, i18n));
    col = col.push(group_bar::view(table, tokens, i18n));
    if table.cache.lines.is_empty() {
        col = col.push(empty_table(
            table,
            &state.components_panel.filter,
            tokens,
            i18n,
        ));
    } else {
        let grid = column![
            header::view(table, previews, tokens, i18n),
            body::view(state, thumb_look, tokens, i18n)
        ]
        .spacing(0)
        .height(Length::Fill);
        // While a column is being resized, the whole table follows the
        // pointer, so a drag that leaves the header strip still resizes.
        // The wrapper is always there, so the body keeps its scroll state.
        let mut area = iced::widget::mouse_area(grid);
        if table.resize.is_some() {
            area = area
                .on_move(|p| msg(ComponentsTableMsg::ResizeMoved(p.x)))
                .on_release(msg(ComponentsTableMsg::ResizeEnded));
        }
        col = col.push(area.boxed());
    }
    col.push(status_line(table, tokens, i18n))
        .height(Length::Fill)
        .boxed()
}

/// "Stock library not found…" while the bundled stock library is missing,
/// in the status line's look; an empty slot otherwise.
fn stock_notice<'a>(
    state: &LibraryState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    if !state.is_stock_missing() {
        return iced::widget::Space::new().height(0.0).boxed();
    }
    container(
        text(t!(i18n, ids::PANELS_COMPONENTS_EMPTY_STOCK))
            .size(HINT_TEXT_SIZE)
            .color(resolve(tokens).ink3),
    )
    .padding([SP.xs, 10.0])
    .width(Length::Fill)
    .boxed()
}

/// "Results 120 of 340 · Sort: Voltage ↑, Value ↓".
fn status_line<'a>(
    table: &'a ComponentsTable,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let cache = &table.cache;
    let (shown, total) = (cache.shown.len(), cache.base.len());
    let results = if shown == total {
        t!(i18n, ids::PANELS_COMPONENTS_RESULTS, "count" => shown)
    } else {
        t!(i18n, ids::PANELS_COMPONENTS_RESULTS_OF, "count" => shown, "total" => total)
    };
    let sort = cache
        .setup
        .sort
        .iter()
        .map(|k| {
            let arrow = if k.descending { "↓" } else { "↑" };
            format!("{} {arrow}", caption(&k.column, i18n))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sort = if sort.is_empty() {
        t!(i18n, ids::PANELS_COMPONENTS_UNSORTED)
    } else {
        t!(i18n, ids::PANELS_COMPONENTS_SORTED_BY, "keys" => sort)
    };
    let muted = resolve(tokens).ink3;
    container(
        row![
            text(results).size(HINT_TEXT_SIZE).color(muted),
            iced::widget::Space::new().width(Length::Fill),
            text(sort)
                .size(HINT_TEXT_SIZE)
                .color(muted)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, 10.0])
    .width(Length::Fill)
    .boxed()
}

/// No part to show: why, with the actions that bring parts back (Clear
/// search, Clear filters, Search all parts).
fn empty_table<'a>(
    table: &'a ComponentsTable,
    filter: &str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let muted = resolve(tokens).ink3;
    let filtered = table.filters().is_some_and(|f| f.active_count() > 0);
    let searched = !filter.trim().is_empty();
    let why = if searched {
        t!(i18n, ids::PANELS_COMPONENTS_NO_MATCH, "filter" => filter.trim().to_owned())
    } else {
        t!(i18n, ids::PANELS_COMPONENTS_NO_PARTS_MATCH)
    };
    let mut col = column![text(why).size(ROW_TEXT_SIZE).color(muted)]
        .spacing(SP.s)
        .align_x(Alignment::Center);
    let action =
        |label: signex_i18n::Localized, pressed: LibraryMessage| -> Element<'a, LibraryMessage> {
            button(text(label).size(HINT_TEXT_SIZE).color(ti(tokens.text)))
                .padding([3.0, 8.0])
                .on_press(pressed)
                .style(crate::styles::outline_button(tokens))
                .boxed()
        };
    if searched {
        col = col.push(action(
            t!(i18n, ids::PANELS_COMPONENTS_CLEAR_FILTER),
            LibraryMessage::ComponentsPanelSetFilter(String::new()),
        ));
    }
    if filtered {
        col = col.push(action(
            t!(i18n, ids::PANELS_COMPONENTS_CLEAR_FILTERS),
            msg(ComponentsTableMsg::FiltersCleared),
        ));
    }
    if !table.category_is_all() {
        col = col.push(action(
            t!(i18n, ids::PANELS_COMPONENTS_SEARCH_ALL),
            msg(ComponentsTableMsg::CategoryPicked(
                super::state::CategoryKey::AllParts,
            )),
        ));
    }
    container(col)
        .padding([SP.l, 10.0])
        .width(Length::Fill)
        .center_x(Length::Fill)
        .boxed()
}

/// A text of a user's data or of catalog text already rendered.
pub(super) fn user_text<'a>(s: impl Into<String>) -> signex_widgets::Label<'a> {
    UserText::new(s.into()).into()
}
