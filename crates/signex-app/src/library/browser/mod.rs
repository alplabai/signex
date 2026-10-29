//! Library Browser tab — the main-window surface for working with
//! library rows.
//!
//! Layout: a table sidebar on the left; on the right a header
//! (search, lifecycle filter) above the row grid and its action row.
//! Row click selects; row detail lives in the right-edge Properties
//! panel. Add Component and Delete Selected are wired through the
//! library messages.

use std::collections::BTreeMap;

use iced::widget::{
    Column, Space, button, column, container, mouse_area, pick_list, row, scrollable, text,
    text_input,
};
use iced::{Border, Element, Length, Theme};
use signex_library::{ComponentRow, LifecycleState, RowId};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::range_filter::range_filter_bar;
use super::state::{LibraryBrowserState, LibraryState, LifecycleFilter, OpenLibrary};

mod action_row;
mod columns;
mod empty_state;
mod grid;
mod header;
mod keyed;
mod price;
mod sidebar;
mod size_sensor;
mod sort;
mod view_cache;
mod window;

use action_row::view_action_row;
use empty_state::view_empty_state;
use grid::view_grid;
use header::view_header;
use sidebar::view_table_sidebar;
pub use view_cache::BrowserViewCache;
use view_cache::active_table;
pub use window::{GridScroll, grid_scroll_id, reveal_row_offset, visible_rows};
pub(crate) use window::{OVERSCAN_ROWS, visible_range};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const BROWSER_TEXT_SIZE: f32 = 11.0;
const BROWSER_HEADER_SIZE: f32 = 10.0;
const MAX_PARAM_COLUMNS: usize = 4;
/// Width reserved at the start of every grid row for the per-row
/// lifecycle indicator dot (Stage 18). The header row uses an empty
/// `Space` of the same width so column labels stay aligned with the
/// row cells beneath them.
const LIFECYCLE_DOT_GUTTER: f32 = 16.0;
const LIFECYCLE_DOT_SIZE: f32 = 8.0;

/// Render the Library Browser tab body. Returns an empty-state panel
/// when the library isn't currently mounted (e.g. mount failed) so the
/// tab still renders without panicking.
pub fn view<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let lib = match library_state.library_at(library_path) {
        Some(lib) => lib,
        None => {
            return container(
                column![
                    text(format!(
                        "Library not mounted: {}",
                        library_path.display()
                    ))
                    .size(TS.title)
                    .color(text_c),
                    Space::new().height(6),
                    text(
                        "Re-open the library through File ▸ Library ▸ Open Library… or via the project tree.",
                    )
                    .size(TS.body)
                    .color(muted),
                ]
                .spacing(0),
            )
            .padding(20)
            .center(Length::Fill)
            .style(crate::styles::modal_card(tokens))
            .into();
        }
    };

    // Read-only mount (the stock library): write affordances render
    // disabled instead of failing on click.
    let read_only = library_state.is_read_only(library_path);

    // Empty library — no tables yet. Show a centred CTA card.
    if lib.tables.is_empty() {
        return view_empty_state(library_path, lib, tokens, read_only);
    }

    // Master-detail layout: left pane lists tables (vertical), right
    // pane = filters/search + grid + actions + preview. Mirrors a DB
    // browser so users can scan their library inventory at a glance
    // and pivot between tables without horizontal scrolling.
    let table_sidebar = view_table_sidebar(library_path, lib, browser, tokens, read_only);
    let header = view_header(library_path, lib, browser, tokens, read_only);

    // Body — left grid, right preview pane.
    let active_table = active_table(lib, browser);
    let rows: &[ComponentRow] = lib
        .tables
        .get(active_table)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    // The filtered + sorted rows and the derived columns come from the
    // cache `update` keeps current; a miss (a path that skipped the sync)
    // recomputes for this frame only.
    let rebuilt;
    let cache = match browser.view_cache.as_ref().filter(|c| {
        c.matches(
            library_state.data_version,
            library_state.clock,
            active_table,
            browser,
        )
    }) {
        Some(cache) => cache,
        None => {
            rebuilt = BrowserViewCache::build(
                lib,
                &library_state.template_registry,
                library_state.clock,
                library_state.data_version,
                browser,
            );
            &rebuilt
        }
    };
    let visible: Vec<&ComponentRow> = cache.visible.iter().filter_map(|&i| rows.get(i)).collect();
    let prices = price::PriceView::with_main_currency(
        &lib.pricing,
        library_state.clock,
        &cache.main_currency,
    );

    let grid = view_grid(
        library_path,
        active_table,
        &visible,
        &cache.columns,
        browser,
        grid::GridCaches {
            dangling: &lib.dangling,
            prices: &prices,
        },
        tokens,
        read_only,
    );

    let actions = view_action_row(
        library_path,
        active_table,
        browser.selected_row,
        tokens,
        read_only,
    );

    let left = container(
        column![grid, actions]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    // F15 (final pass) — the inline preview pane is gone. Row detail
    // and Pick Symbol / Pick Footprint live in the right-edge
    // Properties panel now (`view_library_row_properties`), so the
    // Library Browser tab body keeps the full width for the grid.
    //
    // The Row needs explicit `Length::Fill` width — the
    // `feedback_iced_layout.md` anti-pattern: a Fill child inside a
    // Shrink Row collapses the Row to the child's intrinsic min,
    // which for the Fill grid means the body silently renders empty
    // (visible regression: "double-clicking the .snxlib opens
    // nothing"). The original three-child Row hid this because two
    // of the children were fixed-width.
    let body = row![left].width(Length::Fill).height(Length::Fill);

    // Range filters on the measurement columns (#1572, phase 4).
    let range_bar = range_filter_bar(
        &browser.range_filters,
        &cache.filter_columns,
        &cache.range_excluded,
        tokens,
        {
            let library_path = library_path.to_path_buf();
            move |msg| LibraryMessage::BrowserRangeFilter {
                library_path: library_path.clone(),
                msg,
            }
        },
    )
    .map(|bar| container(bar).padding([4, 10]));

    let mut right = column![header].spacing(0);
    if let Some(bar) = range_bar {
        right = right.push(bar);
    }
    let right = right
        .push(
            container(body)
                .padding(SP.m)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill);

    let border_c = theme_ext::border_color(tokens);
    let separator = container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(border_c)),
            ..iced::widget::container::Style::default()
        });
    row![table_sidebar, separator, right]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
