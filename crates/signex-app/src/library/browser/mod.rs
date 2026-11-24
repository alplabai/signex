//! Library Browser tab — the main-window surface for working with
//! library rows (mockup row 118).
//!
//! Layout: the header line (library, search, lifecycle filter, Add
//! Component) and the uncommitted banner across the top; under them the
//! 210 px sidebar (libraries, tables), the grid with its toolbar row, and
//! the 300 px preview pane of the selected row, whose ⋯ menu holds Delete
//! Selected. `app/view/library_tab.rs` keeps the tab at least
//! [`MIN_BROWSER_W`] wide, scrolling it sideways in a narrower window.

use std::collections::BTreeMap;

use iced::widget::{Column, Space, column, container, mouse_area, row, scrollable};
use iced::{Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, UserText, ids, t};
use signex_library::{ComponentRow, LifecycleState, RowId};
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::theme_ext;
use signex_widgets::{button, raw, text, text_input};

use super::messages::LibraryMessage;
use super::range_filter::range_filter_bar;
use super::state::{LibraryBrowserState, LibraryState, LifecycleFilter, OpenLibrary};
use crate::panels::components_panel::details::symbol_canvas::SheetGrid;

mod columns;
mod empty_state;
mod grid;
mod header;
mod keyed;
mod layout;
mod look;
mod names;
mod preview;
mod price;
mod sidebar;
mod size_sensor;
mod sort;
pub(crate) mod tiles;
mod toolbar;
mod tree;
mod uncommitted;
mod view_cache;
mod window;

use empty_state::view_empty_state;
use grid::view_grid;
use header::view_header;
use uncommitted::view_uncommitted_banner;
pub use view_cache::BrowserViewCache;
use view_cache::active_table;
pub use window::{GridScroll, grid_scroll_id, reveal_row_offset, visible_rows};
pub(crate) use window::{OVERSCAN_ROWS, visible_range};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const BROWSER_TEXT_SIZE: f32 = TS.body;
const BROWSER_HEADER_SIZE: f32 = TS.caption;
const MAX_PARAM_COLUMNS: usize = 4;

/// Narrowest the tab lays out: the sidebar, a usable grid and the preview
/// pane. A narrower window scrolls the tab sideways inside its frame.
pub(crate) const MIN_BROWSER_W: f32 = 1040.0;

/// How the preview pane draws a symbol: the schematic canvas colours and
/// grid, and the window's pixel scale.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PreviewLook {
    pub(crate) canvas_colors: CanvasColors,
    pub(crate) grid: SheetGrid,
    pub(crate) device_px_per_logical_px: f32,
}

#[cfg(test)]
impl PreviewLook {
    /// The Graphite Light canvas with a dotted grid, at 1:1 pixels.
    pub(crate) fn for_tests() -> Self {
        Self {
            canvas_colors: signex_types::theme::canvas_colors(
                signex_types::theme::ThemeId::GraphiteLight,
            ),
            grid: SheetGrid {
                visible: true,
                pitch_mm: 1.27,
                style: crate::render_config::GridStyle::Dots,
            },
            device_px_per_logical_px: 1.0,
        }
    }
}

/// Render the Library Browser tab body. Returns an empty-state panel
/// when the library isn't currently mounted (e.g. mount failed) so the
/// tab still renders without panicking.
pub(crate) fn view<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    look: PreviewLook,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let lib = match library_state.library_at(library_path) {
        Some(lib) => lib,
        None => {
            return container(
                column![
                    text(t!(
                        i18n,
                        ids::LIBRARY_BROWSER_NOT_MOUNTED,
                        "path" => library_path.display().to_string()
                    ))
                    .size(TS.title)
                    .color(text_c),
                    Space::new().height(SP.s),
                    text(t!(i18n, ids::LIBRARY_BROWSER_REOPEN_HINT))
                        .size(TS.body)
                        .color(muted),
                ]
                .spacing(0),
            )
            .padding(SP.xl)
            .center(Length::Fill)
            .style(crate::styles::modal_card(tokens))
            .boxed();
        }
    };

    // Read-only mount (the stock library): write affordances render
    // disabled instead of failing on click.
    let read_only = library_state.is_read_only(library_path);
    let banner = view_uncommitted_banner(library_path, library_state, tokens, i18n);

    // Empty library — no tables yet. Show a centred CTA card.
    if lib.tables.is_empty() {
        return column![
            banner,
            view_empty_state(library_path, lib, tokens, read_only)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    }

    let header = view_header(library_path, lib, browser, tokens, i18n, read_only);
    let sidebar = sidebar::view_sidebar(
        library_path,
        library_state,
        lib,
        browser,
        tokens,
        i18n,
        read_only,
    );
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
        c.matches_in(
            i18n,
            library_state.data_version,
            library_state.clock,
            active_table,
            browser,
        )
    }) {
        Some(cache) => cache,
        None => {
            rebuilt = BrowserViewCache::build_in(
                i18n,
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
    let count = toolbar::ShownCount {
        shown: visible.len(),
        total: rows.len(),
        sorted_by: browser.sort_by.as_ref().and_then(|sort| {
            cache
                .columns
                .iter()
                .find(|c| c.kind.sort_key() == sort.key)
                .map(|c| c.label.clone())
        }),
    };
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
            names: names::PrimitiveNames::of(lib),
        },
        tokens,
        i18n,
        read_only,
    );

    // Range filters on the measurement columns (#1572, phase 4).
    let range_bar = range_filter_bar(
        &browser.range_filters,
        &cache.filter_columns,
        &cache.range_excluded,
        tokens,
        i18n,
        {
            let library_path = library_path.to_path_buf();
            move |msg| LibraryMessage::BrowserRangeFilter {
                library_path: library_path.clone(),
                msg,
            }
        },
    );
    let toolbar = toolbar::view_toolbar(range_bar, &count, tokens, i18n);
    let grid_area = column![toolbar, grid]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    // The Row needs explicit `Length::Fill` width: a Fill child inside a
    // Shrink Row collapses the Row to the child's intrinsic minimum, and
    // the grid then renders empty.
    let mut body: iced::widget::Row<Element<'a, LibraryMessage>> =
        row![sidebar, grid_area.boxed()].spacing(0);
    if let Some(selected) = browser.selected_row
        && let Some(picked) = rows.iter().find(|r| RowId::from_uuid(r.row_id) == selected)
    {
        let names = names::PrimitiveNames::of(lib);
        body = body.push(preview::view_preview(
            picked,
            &names,
            preview::PreviewSource {
                library_path,
                table: active_table,
                library_state,
                browser,
                read_only,
            },
            look,
            tokens,
            i18n,
        ));
    }
    column![
        header,
        banner,
        body.width(Length::Fill).height(Length::Fill).boxed(),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
