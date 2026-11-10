//! Components Panel: one flat list of every mounted library.
//!
//! Each library row carries its expand chevron, its name, a source badge
//! (STOCK / PROJECT / INSTALLED / GLOBAL) and its component count; expanding
//! it lists its components. Source chips under the search box (All, then
//! each source that has a library, with its library count) filter the list.
//! One "+ Add" button in the header opens a Project / Installed / Global
//! menu whose items run the existing add-library command per source (maintainer
//! decision of 2026-11-08, docs/DECISIONS.md).
//!
//! List order is Stock, Project, Installed, Global; inside a source, mount
//! order. All sources read the same `LibraryState::open_libraries`;
//! `LibraryState::mount_source_for` classifies each library at view time
//! (Project wins over a library that is also globally mounted).
//!
//! Search is a case-insensitive substring filter over `mpn` /
//! `manufacturer` / `internal_pn` / library name.

use std::path::PathBuf;

use iced::widget::{Column, Space, column, container, row};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_widgets::{button, raw, text};

use signex_i18n::{I18n, MessageId, ids, t};
use signex_library::ComponentRow;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsMountSource, LibraryState, OpenLibrary};
use crate::styles::Spacing;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: Spacing = Spacing::DEFAULT;

pub mod details;
pub mod global_prefs;
mod look;
pub mod split;
pub mod stock;

const ROW_TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.body;
/// Hints, counts, manufacturer and empty-state subtext; also the layout unit
/// for the pointer-travel and list-height constants.
const HINT_TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.caption;
/// Pointer travel that turns a row press into a drag.
pub(crate) const DRAG_THRESHOLD: f32 = HINT_TEXT_SIZE;
/// Most rows built per library per frame; the rest are summarised by a
/// "N more — refine search" row so a huge library cannot stall the view.
const MAX_ROWS_PER_LIBRARY: usize = 200;

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
    let counts = SourceCounts::of(&libraries);
    let panel = &state.components_panel;
    let selected = counts.effective(panel.source_filter);
    let i18n = &*ctx.i18n;

    // ── Header: search + "+ Add", its menu, the source chips ─────
    let mut col: Column<Element<'a, LibraryMessage>> = column![]
        .spacing(0)
        .width(Length::Fill)
        .push(look::header_row(
            look::filter_field(&panel.filter, tokens, i18n),
            tokens,
            i18n,
        ));
    if panel.add_menu_open {
        col = col.push(look::add_menu(tokens, i18n));
    }
    col = col.push(look::chip_row(source_chips(
        &counts, selected, tokens, i18n,
    )));
    col = col.push(Space::new().height(6.0).boxed());
    col = col.push(thin_sep(look::separator(tokens)));
    if !state.id_clash_notices.is_empty() {
        col = col.push(id_clash_notices(state, tokens));
    }

    // ── One flat list: Stock, Project, Installed, Global ─────────
    // identifier/data folding, never shown
    // ast-grep-ignore: no-case-in-view
    let needle = panel.filter.trim().to_lowercase();
    let mut shown = 0;
    for (source, lib) in &libraries {
        if selected.is_some_and(|s| s != *source) {
            continue;
        }
        shown += 1;
        col = col.push(view_library_block(
            lib, *source, panel, &needle, tokens, i18n,
        ));
    }
    if shown == 0 {
        let message = match selected {
            Some(source) => empty_source_text(source, project_libs),
            None => ids::PANELS_COMPONENTS_NOTHING_MOUNTED,
        };
        col = col.push(look::empty_line(t!(i18n, message), tokens));
    }

    // List (fixed height) | draggable divider | details (the rest).
    let divider = iced::widget::mouse_area(
        container(Space::new())
            .height(split::HANDLE_HEIGHT)
            .width(Length::Fill)
            .style(crate::styles::resize_handle_edge(tokens, false)),
    )
    .interaction(iced::mouse::Interaction::ResizingVertically)
    .on_press(LibraryMessage::ComponentsPanelSplitDragStarted);

    column![
        container(
            crate::panels::panel_scrollable(col.boxed())
                .style(crate::panels::form_look::scrollbar_style),
        )
        .height(panel.list_height)
        .width(Length::Fill),
        divider,
        container(details::view(
            panel,
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

/// Library count per source, for the chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceCounts {
    all: usize,
    per_source: [usize; 4],
}

impl SourceCounts {
    fn of(libraries: &[(ComponentsMountSource, &OpenLibrary)]) -> Self {
        let mut per_source = [0; 4];
        for (source, _) in libraries {
            per_source[Self::slot(*source)] += 1;
        }
        Self {
            all: libraries.len(),
            per_source,
        }
    }

    fn slot(source: ComponentsMountSource) -> usize {
        ComponentsMountSource::ORDER
            .iter()
            .position(|s| *s == source)
            .unwrap_or(0)
    }

    fn of_source(&self, source: ComponentsMountSource) -> usize {
        self.per_source[Self::slot(source)]
    }

    /// The chip choice that applies: a source with no library left falls
    /// back to All.
    fn effective(&self, chosen: Option<ComponentsMountSource>) -> Option<ComponentsMountSource> {
        chosen.filter(|s| self.of_source(*s) > 0)
    }

    /// Chips to draw: All always, then each source that has a library.
    fn chips(&self) -> Vec<Option<ComponentsMountSource>> {
        std::iter::once(None)
            .chain(
                ComponentsMountSource::ORDER
                    .iter()
                    .copied()
                    .filter(|s| self.of_source(*s) > 0)
                    .map(Some),
            )
            .collect()
    }
}

fn source_chips<'a>(
    counts: &SourceCounts,
    selected: Option<ComponentsMountSource>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, LibraryMessage>> {
    counts
        .chips()
        .into_iter()
        .map(|chip| {
            let (label, count) = match chip {
                None => (ids::PANELS_COMPONENTS_SOURCE_ALL, counts.all),
                Some(source) => (source.message(), counts.of_source(source)),
            };
            look::source_chip(
                t!(i18n, label),
                count,
                chip == selected,
                LibraryMessage::ComponentsPanelSourceFilterChanged(chip),
                tokens,
                i18n,
            )
        })
        .collect()
}

/// Non-modal list of libraries refused or closed for sharing a library id
/// with a stronger source (#1908), with one Dismiss button.
fn id_clash_notices<'a>(
    state: &'a LibraryState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let mut notices: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.xs);
    for notice in &state.id_clash_notices {
        notices = notices.push(
            text(raw(notice.message()))
                .size(ROW_TEXT_SIZE)
                .color(text_c)
                .boxed(),
        );
    }
    let dismiss = button(text(raw("Dismiss")).size(ROW_TEXT_SIZE).color(text_c))
        .padding([SP.xs, SP.m])
        .on_press(LibraryMessage::IdClashNoticesDismissed)
        .style(crate::styles::menu_item(tokens));
    container(notices.push(dismiss.boxed()))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// What the workspace says about project libraries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectLibraries {
    NoProject,
    /// A project is open and declares no library.
    NoneDeclared,
    /// A project declares libraries, none of them mounted (pending, missing).
    DeclaredNotMounted,
}

impl ProjectLibraries {
    /// `declared` is the library count each open project declares.
    fn of(declared: &[usize]) -> Self {
        if declared.is_empty() {
            Self::NoProject
        } else if declared.iter().all(|&n| n == 0) {
            Self::NoneDeclared
        } else {
            Self::DeclaredNotMounted
        }
    }
}

/// What an empty source says. Project tells apart no project,
/// a project without libraries and a project whose libraries are not mounted.
fn empty_source_text(src: ComponentsMountSource, project: ProjectLibraries) -> MessageId {
    match src {
        ComponentsMountSource::Project => match project {
            ProjectLibraries::NoProject => ids::PANELS_COMPONENTS_EMPTY_NO_PROJECT,
            ProjectLibraries::NoneDeclared => ids::PANELS_COMPONENTS_EMPTY_NONE_DECLARED,
            ProjectLibraries::DeclaredNotMounted => ids::PANELS_COMPONENTS_EMPTY_NOT_MOUNTED,
        },
        ComponentsMountSource::Installed => ids::PANELS_COMPONENTS_EMPTY_INSTALLED,
        ComponentsMountSource::Global => ids::PANELS_COMPONENTS_EMPTY_GLOBAL,
        ComponentsMountSource::Stock => ids::PANELS_COMPONENTS_EMPTY_STOCK,
    }
}

/// The left label of a component row: part number, then the MPN when set.
fn component_row_label(internal_pn: &str, mpn: &str) -> String {
    match mpn.trim() {
        "" => internal_pn.to_owned(),
        mpn => format!("{internal_pn} — {mpn}"),
    }
}

/// Render one mounted library: its chevron row (name, source badge,
/// component count), then, while expanded, every component matching the
/// panel-wide filter.
fn view_library_block<'a>(
    lib: &'a OpenLibrary,
    source: ComponentsMountSource,
    panel: &'a crate::library::state::ComponentsPanelState,
    needle: &str,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = crate::styles::tokens::resolve(tokens).ink4;
    // Graphite selects with the neutral selected token, not the accent wash.
    let tint = crate::styles::ti(tokens.selection);
    let hover = crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover);
    let row_look = RowLook::graphite();

    let collapsed = panel.collapsed_libraries.contains(&lib.root);
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(0);
    col = col.push(look::library_header(
        lib.root.clone(),
        lib.display_name.clone(),
        source,
        lib.total_rows(),
        collapsed,
        tokens,
        i18n,
    ));
    if collapsed {
        return col.boxed();
    }

    // Rows come from the cache `update` keeps current (lowercase keys and
    // the filtered list), so a frame does no per-row string work and
    // builds widgets for at most `MAX_ROWS_PER_LIBRARY` rows (#1353).
    let rows = &panel.rows;
    let matched = rows.visible_count(&lib.root);
    for (table, index) in rows.visible_rows(&lib.root, MAX_ROWS_PER_LIBRARY) {
        let Some(r) = lib.tables.get(table).and_then(|t| t.get(index)) else {
            continue;
        };
        let key = crate::library::state::ComponentsPanelRowKey {
            library_path: lib.root.clone(),
            table: table.to_owned(),
            row_id: signex_library::RowId::from_uuid(r.row_id),
        };
        let selected = panel.selected.as_ref() == Some(&key);
        let hovered = panel.hovered.as_ref() == Some(&key);
        let colors = RowColors {
            text: text_c,
            muted,
            tint,
            hover,
        };
        col = col.push(view_row_button(key, r, selected, hovered, colors, row_look));
    }

    if matched > MAX_ROWS_PER_LIBRARY {
        col = col.push(
            container(
                text(raw(format!(
                    "{} more — refine search",
                    matched - MAX_ROWS_PER_LIBRARY
                )))
                .size(HINT_TEXT_SIZE)
                .color(muted),
            )
            .padding([SP.xs, row_look.inset])
            .boxed(),
        );
    }

    if matched == 0 && !needle.is_empty() {
        col = col.push(
            container(
                text(raw("(no matches in this library)"))
                    .size(HINT_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.xs, row_look.inset])
            .boxed(),
        );
    }

    col.boxed()
}

/// Height, inset and part-number face of a Components panel row.
#[derive(Debug, Clone, Copy)]
struct RowLook {
    height: f32,
    /// Left inset.
    inset: f32,
    /// Right inset.
    right: f32,
    /// Face of the part number.
    part_font: iced::Font,
}

impl RowLook {
    fn graphite() -> Self {
        Self {
            height: look::ROW_HEIGHT,
            inset: look::ROW_INSET,
            right: look::EDGE,
            part_font: look::mono(),
        }
    }
}

/// The theme colours a Components panel row is drawn with.
#[derive(Debug, Clone, Copy)]
struct RowColors {
    text: Color,
    muted: Color,
    /// Selection fill.
    tint: Color,
    /// Hover fill.
    hover: Color,
}

fn view_row_button<'a>(
    key: crate::library::state::ComponentsPanelRowKey,
    row_data: &'a ComponentRow,
    selected: bool,
    hovered: bool,
    colors: RowColors,
    row_look: RowLook,
) -> Element<'a, LibraryMessage> {
    let RowColors {
        text: text_c,
        muted,
        tint,
        hover,
    } = colors;
    let label_left = component_row_label(row_data.internal_pn.as_str(), &row_data.primary_mpn.mpn);
    let label_right = row_data.primary_mpn.manufacturer.clone();

    let part = text(signex_i18n::UserText::new(label_left))
        .size(ROW_TEXT_SIZE)
        .color(text_c)
        .width(Length::FillPortion(3))
        .wrapping(iced::widget::text::Wrapping::None)
        .font(row_look.part_font);
    let content = container(
        row![
            part,
            text(raw(label_right))
                .size(HINT_TEXT_SIZE)
                .color(muted)
                .width(Length::FillPortion(2))
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(iced::Padding {
        left: row_look.inset,
        right: row_look.right,
        ..iced::Padding::ZERO
    })
    .height(row_look.height)
    .center_y(row_look.height);
    // A press selects the row and arms a drag onto the schematic; a
    // right-click selects it and opens its menu. A row never opens a tab.
    let content = content.style(move |_: &Theme| container::Style {
        background: if selected {
            Some(Background::Color(tint))
        } else {
            hovered.then_some(Background::Color(hover))
        },
        ..container::Style::default()
    });

    iced::widget::mouse_area(content)
        .on_press(LibraryMessage::ComponentsPanelRowPressed(key.clone()))
        .on_right_press(LibraryMessage::ComponentsPanelRowRightClicked(key.clone()))
        .on_enter(LibraryMessage::ComponentsPanelRowEntered(key.clone()))
        .on_exit(LibraryMessage::ComponentsPanelRowExited(key))
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
mod tests {
    use super::*;
    use signex_library::{
        ComponentClass, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart, ParamMap,
        PinPadOverride, PlmReserved,
    };
    use uuid::Uuid;

    pub(super) fn fixture_row(pn: &str, mpn: &str, mfr: &str) -> ComponentRow {
        let _ = (PinPadOverride::new("1", "1"),);
        ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::now_v7(),
            internal_pn: InternalPn::new(pn),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::now_v7()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft(mfr, mpn),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
        }
    }

    /// More rows than the cap: the view builds the cap's worth and says how
    /// many it left out; the rest stay reachable by narrowing the filter.
    #[test]
    fn rows_beyond_the_cap_collapse_into_a_refine_search_row() {
        let total = 250;
        let mut state = LibraryState::default();
        let rows = (0..total)
            .map(|i| fixture_row(&format!("PN-{i}"), "MPN", "Mfr"))
            .collect();
        state.open_libraries.push(OpenLibrary {
            root: PathBuf::from("/tmp/big.snxlib"),
            display_name: "big".into(),
            library_id: uuid::Uuid::now_v7(),
            tables: std::collections::HashMap::from([("t".to_owned(), rows)]),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
        });
        state.touch_data();
        state.sync_view_caches();

        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let window = iced::Size::new(600.0, 100_000.0);
        fn build<'a>(
            state: &'a LibraryState,
            tokens: &'a ThemeTokens,
        ) -> Element<'a, LibraryMessage> {
            view_library_block(
                &state.open_libraries[0],
                ComponentsMountSource::Stock,
                &state.components_panel,
                "",
                tokens,
                &signex_i18n::I18n::en_us(),
            )
        }
        let texts = crate::layout_probe::lay_out_texts(build(&state, &tokens), window);
        let rows = texts.iter().filter(|t| t.text.starts_with("PN-")).count();
        assert_eq!(rows, 200);
        crate::layout_probe::find(&texts, "50 more — refine search");

        // Narrowing the filter below the cap removes the summary row.
        state.components_panel.filter = "pn-7".into();
        state.sync_view_caches();
        let texts = crate::layout_probe::lay_out_texts(build(&state, &tokens), window);
        assert!(texts.iter().all(|t| !t.text.contains("refine search")));
        assert_eq!(
            texts.iter().filter(|t| t.text.starts_with("PN-")).count(),
            11
        );
    }

    /// One library holding `rows`, its view caches current.
    pub(super) fn library_with(rows: Vec<ComponentRow>) -> LibraryState {
        let mut state = LibraryState::default();
        state.open_libraries.push(OpenLibrary {
            root: PathBuf::from("/tmp/one.snxlib"),
            display_name: "one".into(),
            library_id: uuid::Uuid::now_v7(),
            tables: std::collections::HashMap::from([("t".to_owned(), rows)]),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
        });
        state.touch_data();
        state.sync_view_caches();
        state
    }

    /// Graphite: the library header and every row are 26 px tall, the rows
    /// one indent step in from their library, and the selected row is filled
    /// with the theme's selected token, not the accent wash.
    #[test]
    fn graphite_rows_are_dense_indented_and_select_with_the_selected_token() {
        use signex_types::theme::{ThemeId, theme_tokens};
        let mut state = library_with(vec![
            fixture_row("PN-A", "", "Mfr"),
            fixture_row("PN-B", "", "Mfr"),
        ]);
        let row_id = state.open_libraries[0].tables["t"][0].row_id;
        state.components_panel.selected = Some(crate::library::state::ComponentsPanelRowKey {
            library_path: PathBuf::from("/tmp/one.snxlib"),
            table: "t".into(),
            row_id: signex_library::RowId::from_uuid(row_id),
        });
        let window = iced::Size::new(400.0, 200.0);
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let build = || {
                view_library_block(
                    &state.open_libraries[0],
                    ComponentsMountSource::Stock,
                    &state.components_panel,
                    "",
                    &tokens,
                    &signex_i18n::I18n::en_us(),
                )
            };
            let texts = crate::layout_probe::lay_out_texts(build(), window);
            let header = crate::layout_probe::find(&texts, "one").bounds;
            let a = crate::layout_probe::find(&texts, "PN-A").bounds;
            let b = crate::layout_probe::find(&texts, "PN-B").bounds;
            assert!(
                (b.y - a.y - look::ROW_HEIGHT).abs() < 0.5,
                "{id:?}: rows are {} px apart",
                b.y - a.y
            );
            assert!(
                (a.x - look::ROW_INSET).abs() < 0.5,
                "{id:?}: row at {}",
                a.x
            );
            assert!(
                (header.x - look::LIBRARY_INSET).abs() < 0.5,
                "{id:?}: header at {}",
                header.x
            );

            let pixels = crate::layout_probe::draw_pixels(build(), window);
            let at = |x: usize, y: usize| {
                let i = (y * window.width as usize + x) * 4;
                [pixels[i], pixels[i + 1], pixels[i + 2]]
            };
            let sel = tokens.selection;
            let y = (look::ROW_HEIGHT * 1.5) as usize;
            assert_eq!(at(4, y), [sel.r, sel.g, sel.b], "{id:?}: selected fill");
            assert_ne!(
                at(4, y + look::ROW_HEIGHT as usize),
                [sel.r, sel.g, sel.b],
                "{id:?}: the unselected row is not filled"
            );
        }
    }

    #[test]
    fn mount_sources_list_stock_first_then_project_installed_global() {
        assert_eq!(
            ComponentsMountSource::ORDER,
            &[
                ComponentsMountSource::Stock,
                ComponentsMountSource::Project,
                ComponentsMountSource::Installed,
                ComponentsMountSource::Global,
            ]
        );
    }

    /// Press and right-click a row the way the runtime would. A row has no
    /// buttons: no "Place" / "Open" text, a press selects, a right-click
    /// asks for the menu.
    #[test]
    fn a_row_has_no_buttons_and_publishes_press_and_right_click() {
        use iced::{Event, Point, Size, mouse};
        use iced_runtime::UserInterface;
        use iced_runtime::user_interface::Cache;

        let row = fixture_row("R0805_10k", "ERA-3AED103V", "Panasonic");
        let row_id = signex_library::RowId::from_uuid(row.row_id);
        let window = Size::new(600.0, 200.0);
        let build = || {
            view_row_button(
                crate::library::state::ComponentsPanelRowKey {
                    library_path: PathBuf::from("lib.snxlib"),
                    table: "Passives".to_owned(),
                    row_id,
                },
                &row,
                false,
                false,
                RowColors {
                    text: Color::WHITE,
                    muted: Color::WHITE,
                    tint: Color::BLACK,
                    hover: Color::BLACK,
                },
                RowLook::graphite(),
            )
        };

        let texts = crate::layout_probe::lay_out_texts(build(), window);
        assert!(texts.iter().all(|t| t.text != "Place" && t.text != "Open"));
        let label = crate::layout_probe::find(&texts, "R0805_10k — ERA-3AED103V").bounds;
        let at = Point::new(label.x + label.width / 2.0, label.y + label.height / 2.0);

        let publish = |events: &[Event]| {
            let mut renderer = crate::layout_probe::headless_renderer();
            let mut ui = UserInterface::build(build(), window, Cache::default(), &mut renderer);
            let mut published = iced::advanced::shell::Bus::new();
            let _ = ui.update(
                &iced::window::Headless,
                &iced::advanced::shell::Waker::noop(),
                events,
                mouse::Cursor::Available(at),
                &mut renderer,
                &mut published,
            );
            // Hover enter/exit are not clicks.
            let mut published: Vec<LibraryMessage> = published.into_iter().collect();
            published.retain(|m| {
                !matches!(
                    m,
                    LibraryMessage::ComponentsPanelRowEntered(_)
                        | LibraryMessage::ComponentsPanelRowExited(_)
                )
            });
            published
        };

        let pressed = publish(&[Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Left,
        ))]);
        assert_eq!(pressed.len(), 1, "one press, one message");
        match &pressed[0] {
            LibraryMessage::ComponentsPanelRowPressed(key) => {
                assert_eq!(key.library_path, PathBuf::from("lib.snxlib"));
                assert_eq!(key.table, "Passives");
                assert_eq!(key.row_id, row_id);
            }
            _ => panic!("expected ComponentsPanelRowPressed"),
        }

        let right = publish(&[Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Right,
        ))]);
        assert_eq!(right.len(), 1);
        assert!(matches!(
            right[0],
            LibraryMessage::ComponentsPanelRowRightClicked(_)
        ));
    }

    #[test]
    fn mount_source_label_and_key_distinct() {
        for src in ComponentsMountSource::ORDER {
            assert!(!src.label().is_empty());
            assert!(!src.key().is_empty());
            assert_ne!(src.label(), src.key());
        }
    }

    #[test]
    fn the_project_section_tells_its_three_states_apart() {
        let en = I18n::en_us();
        let project =
            |state| t!(en, empty_source_text(ComponentsMountSource::Project, state)).into_string();
        let none = project(ProjectLibraries::NoProject);
        let empty = project(ProjectLibraries::NoneDeclared);
        let unmounted = project(ProjectLibraries::DeclaredNotMounted);
        assert!(none.contains("Open a project"), "{none}");
        assert!(empty.contains("no libraries yet"), "{empty}");
        assert!(
            empty.contains("Add New to Project > Component Library"),
            "{empty}"
        );
        assert!(unmounted.contains("not mounted"), "{unmounted}");
        assert!(!unmounted.contains("no libraries yet"), "{unmounted}");
    }

    #[test]
    fn the_state_comes_from_the_declared_libraries() {
        assert_eq!(ProjectLibraries::of(&[]), ProjectLibraries::NoProject);
        assert_eq!(ProjectLibraries::of(&[0]), ProjectLibraries::NoneDeclared);
        assert_eq!(
            ProjectLibraries::of(&[0, 2]),
            ProjectLibraries::DeclaredNotMounted
        );
    }

    #[test]
    fn other_sections_ignore_the_project_state() {
        for src in [
            ComponentsMountSource::Installed,
            ComponentsMountSource::Global,
            ComponentsMountSource::Stock,
        ] {
            assert_eq!(
                empty_source_text(src, ProjectLibraries::NoneDeclared),
                empty_source_text(src, ProjectLibraries::NoProject)
            );
        }
    }

    #[test]
    fn a_row_without_an_mpn_drops_the_trailing_dash() {
        assert_eq!(component_row_label("STOCK-R", ""), "STOCK-R");
        assert_eq!(component_row_label("STOCK-R", "  "), "STOCK-R");
        assert_eq!(component_row_label("R0805", "RC0805"), "R0805 — RC0805");
    }

    /// A state with a stock, a global and two installed libraries, mounted
    /// in that scrambled order: global, installed A, stock, installed B.
    fn four_libraries() -> LibraryState {
        let mut state = LibraryState::default();
        for name in ["g", "ia", "s", "ib"] {
            let mut one = library_with(vec![fixture_row("PN", "", "Mfr")]);
            let mut lib = one.open_libraries.remove(0);
            lib.root = PathBuf::from(format!("/tmp/{name}.snxlib"));
            lib.display_name = name.into();
            state.open_libraries.push(lib);
        }
        state.stock_library_root = Some(PathBuf::from("/tmp/s.snxlib"));
        state.installed_libraries = vec![
            PathBuf::from("/tmp/ia.snxlib"),
            PathBuf::from("/tmp/ib.snxlib"),
        ];
        state.global_libraries = vec![global_prefs::GlobalLibraryEntry {
            path: PathBuf::from("/tmp/g.snxlib"),
            remote: None,
            auto_pull: None,
        }];
        state
    }

    fn names(listed: &[(ComponentsMountSource, &OpenLibrary)]) -> Vec<String> {
        listed.iter().map(|(_, l)| l.display_name.clone()).collect()
    }

    /// One flat list, Stock first, then Project, Installed, Global; inside a
    /// source, mount order.
    #[test]
    fn the_flat_list_orders_stock_then_project_installed_global() {
        let state = four_libraries();
        let listed = listed_libraries(&state, &[]);
        assert_eq!(names(&listed), ["s", "ia", "ib", "g"]);
        assert_eq!(listed[0].0, ComponentsMountSource::Stock);
        assert_eq!(listed[3].0, ComponentsMountSource::Global);
    }

    /// "All" is always a chip; a source with no library has none.
    #[test]
    fn empty_source_chips_are_hidden_and_all_stays() {
        let state = four_libraries();
        let counts = SourceCounts::of(&listed_libraries(&state, &[]));
        assert_eq!(
            counts.chips(),
            [
                None,
                Some(ComponentsMountSource::Stock),
                Some(ComponentsMountSource::Installed),
                Some(ComponentsMountSource::Global),
            ],
            "Project has no library, so no chip"
        );
        assert_eq!(counts.all, 4);
        assert_eq!(counts.of_source(ComponentsMountSource::Installed), 2);

        let nothing = SourceCounts::of(&[]);
        assert_eq!(nothing.chips(), [None], "All shows even with no library");
    }

    /// A chip filters the list to its source, and a source that lost its
    /// last library falls back to All instead of showing an empty list.
    #[test]
    fn a_source_chip_filters_the_list_and_an_emptied_one_falls_back_to_all() {
        let state = four_libraries();
        let listed = listed_libraries(&state, &[]);
        let counts = SourceCounts::of(&listed);
        let shown = |chip| {
            let selected = counts.effective(chip);
            names(
                &listed
                    .iter()
                    .filter(|(s, _)| selected.is_none_or(|c| c == *s))
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(shown(None), ["s", "ia", "ib", "g"]);
        assert_eq!(shown(Some(ComponentsMountSource::Installed)), ["ia", "ib"]);
        assert_eq!(shown(Some(ComponentsMountSource::Global)), ["g"]);
        assert_eq!(
            shown(Some(ComponentsMountSource::Project)),
            ["s", "ia", "ib", "g"],
            "Project has no library: the choice no longer applies"
        );
    }

    /// The header "+ Add" menu offers Project, Installed and Global, each
    /// the existing add-library command for that source.
    #[test]
    fn the_add_menu_offers_project_installed_and_global() {
        assert_eq!(
            ComponentsMountSource::ADDABLE,
            &[
                ComponentsMountSource::Project,
                ComponentsMountSource::Installed,
                ComponentsMountSource::Global,
            ]
        );
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let texts = crate::layout_probe::lay_out_texts(
            look::add_menu(&tokens, &I18n::en_us()),
            iced::Size::new(300.0, 300.0),
        );
        for source in ComponentsMountSource::ADDABLE {
            crate::layout_probe::find(&texts, source.label());
        }
    }

    /// A collapsed library keeps its row (name, badge, count) and drops its
    /// components; expanded lists them.
    #[test]
    fn a_collapsed_library_hides_its_components_and_keeps_its_row() {
        let mut state = library_with(vec![fixture_row("PN-A", "", "Mfr")]);
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let window = iced::Size::new(400.0, 200.0);
        let probe = |state: &LibraryState| {
            crate::layout_probe::lay_out_texts(
                view_library_block(
                    &state.open_libraries[0],
                    ComponentsMountSource::Global,
                    &state.components_panel,
                    "",
                    &tokens,
                    &signex_i18n::I18n::en_us(),
                ),
                window,
            )
        };
        let open = probe(&state);
        crate::layout_probe::find(&open, "PN-A");
        crate::layout_probe::find(&open, "GLOBAL");

        let root = state.open_libraries[0].root.clone();
        state.components_panel.collapsed_libraries.insert(root);
        let closed = probe(&state);
        assert!(closed.iter().all(|t| t.text != "PN-A"));
        crate::layout_probe::find(&closed, "one");
        crate::layout_probe::find(&closed, "GLOBAL");
    }
}
