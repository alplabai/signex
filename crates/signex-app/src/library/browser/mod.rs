//! Library Browser tab — the main-window surface for working with
//! library rows.
//!
//! Layout:
//!
//! ```text
//! ┌─[ <name>.snxlib ]──────────────────────────────────────────┐
//! │ [Resistors] [Capacitors] [Connectors] [+]   Search: [____] │  ← tab strip
//! ├──────────────────────────────────────────┬─────────────────┤
//! │ ┌──────┬────────┬──────┬─────┬─────┐     │  [ Preview  ]   │
//! │ │ PN   │ Mfr    │ MPN  │ Val │ Pkg │     │  [   symbol  ]  │
//! │ │ R10K │ Vishay │ CRC… │ 10k │0805 │  ←  │  ───────────── │
//! │ │ R47K │ Yageo  │ RC0… │ 47k │0805 │     │  [ footprint ]  │
//! │ └──────┴────────┴──────┴─────┴─────┘     │                 │
//! │   Add Component  Delete Selected         │                 │
//! └──────────────────────────────────────────┴─────────────────┘
//! ```
//!
//! Phase 1 = read-only-plus-modal-edit semantics. The grid is rendered
//! as `text` widgets; row click selects (drives the side preview pane);
//! row double-click is reserved for the upcoming Edit Component Details
//! modal (Phase 2). Add Component and Delete Selected are wired through
//! the existing library messages; Delete fires immediately without a
//! confirm modal until Phase 2 lands.

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
use super::state::{LibraryBrowserState, LibraryState, LifecycleFilter, OpenLibrary};

mod action_row;
mod columns;
mod empty_state;
mod grid;
mod header;
mod preview;
mod sidebar;

use action_row::view_action_row;
use columns::{compare_cells, derive_columns, row_matches_filter};
use empty_state::view_empty_state;
use grid::view_grid;
use header::view_header;
use sidebar::view_table_sidebar;

const BROWSER_TEXT_SIZE: f32 = 11.0;
const BROWSER_HEADER_SIZE: f32 = 10.0;
#[allow(dead_code)] // F15 (final): preview pane removed; constant retained for the moment in case the Properties panel needs the same width hint.
const PREVIEW_PANE_WIDTH: f32 = 380.0;
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
                    .size(13)
                    .color(text_c),
                    Space::new().height(6),
                    text(
                        "Re-open the library through File ▸ Library ▸ Open Library… or via the project tree.",
                    )
                    .size(11)
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

    // Empty library — no tables yet. Show a centred CTA card.
    if lib.tables.is_empty() {
        return view_empty_state(library_path, lib, tokens);
    }

    // Master-detail layout: left pane lists tables (vertical), right
    // pane = filters/search + grid + actions + preview. Mirrors a DB
    // browser so users can scan their library inventory at a glance
    // and pivot between tables without horizontal scrolling.
    let table_sidebar = view_table_sidebar(library_path, library_state, lib, browser, tokens);
    let header = view_header(library_path, lib, browser, tokens);

    // Body — left grid, right preview pane.
    let active_table = browser.active_table.as_deref().unwrap_or_else(|| {
        // Fall back to first sorted table name when active_table is
        // unset — matches the open-tab handler's seeding logic.
        let mut names: Vec<&String> = lib.tables.keys().collect();
        names.sort();
        names.first().map(|s| s.as_str()).unwrap_or("")
    });

    let rows: &[ComponentRow] = lib
        .tables
        .get(active_table)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let needle = browser.search.trim().to_lowercase();
    let lifecycle_filter = browser.lifecycle_filter;
    let class_filter = browser.class_filter.as_deref();
    let mut visible: Vec<&ComponentRow> = rows
        .iter()
        .filter(|r| lifecycle_filter.allows(r.state))
        .filter(|r| class_filter.map_or(true, |cls| r.class.as_str() == cls))
        .filter(|r| needle.is_empty() || row_matches_filter(r, &needle))
        .collect();

    let columns = derive_columns(
        rows,
        lib.library_id,
        &library_state.template_registry,
        active_table,
    );

    // Stage 8: apply the user's sort selection to the visible rows
    // before grid rendering. The grid view is a pure projection of
    // `visible`, so sorting here doesn't ripple into the render path.
    if let Some(sort) = browser.sort_by.as_ref() {
        if let Some(column) = columns.iter().find(|c| c.kind.sort_key() == sort.key) {
            visible.sort_by(|a, b| {
                let ca = column.kind.cell_value(a);
                let cb = column.kind.cell_value(b);
                let ord = compare_cells(&ca, &cb);
                if sort.descending { ord.reverse() } else { ord }
            });
        }
    }

    let grid = view_grid(
        library_path,
        active_table,
        &visible,
        &columns,
        browser,
        tokens,
    );

    let actions = view_action_row(library_path, active_table, browser.selected_row, tokens);

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
    // `library_state` and `view_preview_pane` retained but unused
    // here; remove in a follow-up cleanup pass once the Properties-
    // panel approach is locked.
    //
    // The Row needs explicit `Length::Fill` width — the
    // `feedback_iced_layout.md` anti-pattern: a Fill child inside a
    // Shrink Row collapses the Row to the child's intrinsic min,
    // which for the Fill grid means the body silently renders empty
    // (visible regression: "double-clicking the .snxlib opens
    // nothing"). The original three-child Row hid this because two
    // of the children were fixed-width.
    let _ = library_state;
    let body = row![left].width(Length::Fill).height(Length::Fill);

    let right = column![
        header,
        container(body)
            .padding(8)
            .width(Length::Fill)
            .height(Length::Fill)
    ]
    .spacing(0)
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
