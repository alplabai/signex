//! Components Panel: one flat list of every mounted library.
//!
//! Each library section carries its expand chevron, its name, a source badge
//! (STOCK / PROJECT / INSTALLED / GLOBAL) and its part count ("shown of
//! total" while filtering); expanding it lists its parts. Source chips under
//! the search box (All, then each source holding parts, with its part count)
//! filter the list. One "+ Add" button in the header opens a Project /
//! Installed / Global menu whose items run the existing add-library command
//! per source (maintainer decision of 2026-11-08, docs/DECISIONS.md); the
//! "Previews" switch beside it picks Compact or Previews rows (#2535).
//!
//! List order is Stock, Project, Installed, Global; inside a source, mount
//! order. All sources read the same `LibraryState::open_libraries`;
//! `LibraryState::mount_source_for` classifies each library at view time
//! (Project wins over a library that is also globally mounted).
//!
//! With a part selected the splitter divides the list from its details;
//! with none, the list takes the free height above Recently placed.
//!
//! Search is value-aware (`query`): `100n` finds a 0.1 µF part, a bare
//! `100` every measurement showing 100, and other words are case-insensitive
//! substrings of the part number, MPN, manufacturer, description or library
//! name. An info line under the header counts what each bare number matched.

use std::path::PathBuf;

use iced::widget::{Column, Space, column, container};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_widgets::{button, raw, text};

use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsMountSource, LibraryState, OpenLibrary};
use crate::styles::Spacing;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: Spacing = Spacing::DEFAULT;

mod chips;
pub mod details;
mod empty;
pub mod global_prefs;
mod look;
pub(crate) mod query;
pub mod recent;
mod rows;
mod sections;
pub mod split;
pub mod stock;
pub mod thumbs;

use chips::SourceCounts;
use empty::ProjectLibraries;
use sections::view_library_block;

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
///
/// The Project mount-source bucket reads through
/// `ctx.projects[].libraries[].root` rather than threading a
/// separate slice. The panel context already carries the project
/// info, so the dock's `view_region` signature stays narrow.
pub(crate) fn view<'a>(
    state: &'a LibraryState,
    ctx: &'a crate::panels::PanelContext,
    tokens: &'a ThemeTokens,
    grid: details::symbol_canvas::SheetGrid,
) -> Element<'a, LibraryMessage> {
    // Derive Project library paths from the panel context. Each
    // `ProjectPanelInfo.libraries[].root` is the absolute, resolved
    // `.snxlib` path the loader handed off to the auto-mount step.
    let project_libs = ProjectLibraries::of(
        &ctx.projects
            .iter()
            .map(|p| p.libraries.len())
            .collect::<Vec<_>>(),
    );
    let project_paths: Vec<PathBuf> = ctx
        .projects
        .iter()
        .flat_map(|p| p.libraries.iter().map(|l| l.root.clone()))
        .collect();
    let libraries = listed_libraries(state, &project_paths);
    let panel = &state.components_panel;
    let counts = SourceCounts::of(&libraries, &panel.rows);
    let selected = counts.effective(panel.source_filter);
    let i18n = &*ctx.i18n;
    let thumb_look = thumbs::ThumbLook {
        colors: ctx.canvas_colors,
        device_px_per_logical_px: ctx.device_px_per_logical_px,
    };

    // ── Header: search, "+ Add", Previews; the add menu; the chips ─
    let mut col: Column<Element<'a, LibraryMessage>> = column![]
        .spacing(0)
        .width(Length::Fill)
        .push(look::header_row(
            look::filter_field(&panel.filter, tokens, i18n),
            panel.row_style,
            tokens,
            i18n,
        ));
    if panel.add_menu_open {
        col = col.push(look::add_menu(tokens, i18n));
    }
    if !libraries.is_empty() {
        col = col.push(chips::chip_row(&counts, selected, tokens, i18n));
        col = col.push(Space::new().height(6.0).boxed());
        col = col.push(thin_sep(look::separator(tokens)));
    }
    if let Some(summary) = look::match_summary(panel.rows.match_summary(), tokens, i18n) {
        col = col.push(summary);
    }
    if !state.id_clash_notices.is_empty() {
        col = col.push(id_clash_notices(state, tokens, i18n));
    }

    // ── One flat list: Stock, Project, Installed, Global ─────────
    let needle = i18n.fold_for_search(panel.filter.trim());
    let shown_parts = selected.map_or(counts.all, |s| counts.of_source(s));
    if libraries.is_empty() {
        col = col.push(empty::nothing_mounted(tokens, i18n));
    } else if !needle.is_empty() && shown_parts == 0 {
        col = col.push(empty::no_match(&panel.filter, tokens, i18n));
    } else {
        let mut shown = 0;
        for (source, lib) in &libraries {
            if selected.is_some_and(|s| s != *source) {
                continue;
            }
            shown += 1;
            col = col.push(view_library_block(
                lib, *source, panel, &needle, thumb_look, tokens, i18n,
            ));
        }
        if let (0, Some(source)) = (shown, selected) {
            let message = empty::empty_source_text(source, project_libs);
            col = col.push(look::empty_line(t!(i18n, message), tokens));
        }
    }

    let list = container(
        crate::panels::panel_scrollable(col.boxed())
            .style(crate::panels::form_look::scrollbar_style),
    )
    .width(Length::Fill);

    // Nothing selected: the list takes the free height above Recently
    // placed. A part selected: the splitter sets the list height.
    if recent::shows_recent(panel) {
        return column![
            list.height(Length::Fill),
            thin_sep(look::separator(tokens)),
            recent::view(state, panel, tokens, thumb_look, i18n),
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    }
    let divider = split::handle(panel.split_hovered, panel.split_dragging, tokens);
    let library_name = panel
        .selected
        .as_ref()
        .and_then(|key| state.library_at(&key.library_path))
        .map(|lib| lib.display_name.as_str());

    column![
        list.height(panel.list_height),
        divider,
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
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// Every mounted library with its source, in list order: Stock, Project,
/// Installed, Global; within a source, in mount order.
fn listed_libraries<'a>(
    state: &'a LibraryState,
    project_paths: &[PathBuf],
) -> Vec<(ComponentsMountSource, &'a OpenLibrary)> {
    let mut listed: Vec<(ComponentsMountSource, &'a OpenLibrary)> = state
        .open_libraries
        .iter()
        .map(|lib| (state.mount_source_for(&lib.root, project_paths), lib))
        .collect();
    // Stable: mount order survives inside a source.
    listed.sort_by_key(|(source, _)| {
        ComponentsMountSource::ORDER
            .iter()
            .position(|s| s == source)
            .unwrap_or(usize::MAX)
    });
    listed
}

/// The symbol references the panel would draw, in list order: what the
/// thumbnail read is asked for. The dragged row's card always shows one;
/// the rows and Recently placed only in Previews.
pub(crate) fn wanted_thumbnails(
    state: &LibraryState,
) -> Vec<signex_library::primitive::PrimitiveRef> {
    let panel = &state.components_panel;
    let dragged = panel
        .drag
        .as_ref()
        .and_then(|drag| recent::row_of(state, &drag.key))
        .map(|row| row.symbol_ref);
    if !panel.row_style.shows_previews() {
        return dragged.into_iter().collect();
    }
    let recent = recent::listed(state, panel)
        .into_iter()
        .map(|(_, row)| row.symbol_ref);
    let rows = state
        .open_libraries
        .iter()
        .filter(|lib| !panel.collapsed_libraries.contains(&lib.root))
        .flat_map(|lib| {
            panel
                .rows
                .visible_rows(&lib.root, sections::MAX_ROWS_PER_LIBRARY)
                .filter_map(move |(table, index)| {
                    Some(lib.tables.get(table)?.get(index)?.symbol_ref)
                })
        });
    dragged.into_iter().chain(recent).chain(rows).collect()
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
mod order_tests;

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "rows_tests.rs"]
mod rows_tests;
