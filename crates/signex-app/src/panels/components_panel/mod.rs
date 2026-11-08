//! Components Panel — Stage 9 of `v0.9-snxlib-as-file-plan.md`.
//!
//! The panel surfaces every mounted library through three collapsible
//! sections, classified by mount source:
//!
//! 1. **Project** — auto-mounted from the active workspace's
//!    `Project.libraries` (existing flow, see
//!    [`crate::library::commands::auto_mount_project_libraries`]).
//! 2. **Installed** — session-scoped, opened via the "+ Add Library…"
//!    button on the Installed section header. Wiped on app close.
//! 3. **Global** — persisted across launches via
//!    `<config_dir>/signex/global_libraries.toml`. Loaded at startup by
//!    [`global_prefs::load`] and mounted off the UI thread.
//!
//! All three sources read from the same `LibraryState::open_libraries`
//! Vec — `LibraryState::mount_source_for` does the bucketing at view
//! time. The bucketing avoids double-rendering a library that's both
//! a project library and globally mounted (Project wins).
//!
//! Stage 9 ships scaffold + data model + simple substring filter on
//! `mpn` / `manufacturer` / `internal_pn` / library name. The rich
//! search syntax (`mpn:LM317 lifecycle:preferred rated_power_mw>=500`,
//! plan §5) and the ghost-component drag-to-place are polish work for
//! a later stage.

use std::path::PathBuf;

use iced::widget::{Column, Space, button, column, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};

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

const PANEL_TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.body;
const SECTION_HEADER_TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.label;
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
    let muted = theme_ext::text_secondary(tokens);
    let graphite = crate::styles::is_graphite(tokens);
    let border_c = if graphite {
        look::separator(tokens)
    } else {
        theme_ext::border_color(tokens)
    };

    // ── Header: filter input ─────────────────────────────────────
    let filter_row = if graphite {
        look::filter_field(&state.components_panel.filter, tokens)
    } else {
        let filter = text_input("Filter components…", &state.components_panel.filter)
            .on_input(LibraryMessage::ComponentsPanelSetFilter)
            .padding(SP.s)
            .size(PANEL_TEXT_SIZE);
        container(filter).padding([SP.s, 6.0]).boxed()
    };

    // ── Three collapsible source sections ────────────────────────
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![].spacing(0).width(Length::Fill).push(filter_row);
    if !state.id_clash_notices.is_empty() {
        col = col.push(id_clash_notices(state, tokens));
    }

    // Derive Project library paths from the panel context. Each
    // `ProjectPanelInfo.libraries[].root` is the absolute, resolved
    // `.snxlib` path the loader handed off to the auto-mount step.
    // Bucket every mounted library up-front so view_section
    // doesn't have to borrow back into the per-call Vec
    // `project_paths` (which can't live for the panel's `'a`).
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
    let mut bucketed: [Vec<&'a OpenLibrary>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for lib in &state.open_libraries {
        let src = state.mount_source_for(&lib.root, &project_paths);
        let idx = match src {
            ComponentsMountSource::Project => 0,
            ComponentsMountSource::Installed => 1,
            ComponentsMountSource::Global => 2,
            ComponentsMountSource::Stock => 3,
        };
        bucketed[idx].push(lib);
    }
    drop(project_paths);

    for (i, src) in ComponentsMountSource::ORDER.iter().enumerate() {
        col = col.push(view_section(
            *src,
            state,
            &bucketed[i],
            project_libs,
            tokens,
        ));
        col = col.push(thin_sep(border_c));
    }

    // Empty state — every section is empty AND no libraries mounted
    const NOTHING_MOUNTED: &str = "No libraries mounted. Use \"+ Add Library…\" or open a project.";
    if state.open_libraries.is_empty() && graphite {
        col = col.push(look::empty_line(NOTHING_MOUNTED, tokens));
    } else if state.open_libraries.is_empty() {
        col = col.push(
            container(text(NOTHING_MOUNTED).size(PANEL_TEXT_SIZE).color(muted))
                .padding([10.0, SP.m])
                .boxed(),
        );
    }

    // List (fixed height) | draggable divider | details (the rest).
    let divider_look: Box<dyn Fn(&Theme) -> container::Style> = if graphite {
        Box::new(crate::styles::resize_handle_edge(tokens, false))
    } else {
        Box::new(crate::styles::resize_handle(tokens))
    };
    let divider = iced::widget::mouse_area(
        container(Space::new())
            .height(split::HANDLE_HEIGHT)
            .width(Length::Fill)
            .style(divider_look),
    )
    .interaction(iced::mouse::Interaction::ResizingVertically)
    .on_press(LibraryMessage::ComponentsPanelSplitDragStarted);

    column![
        container(
            crate::panels::panel_scrollable(col.boxed())
                .style(crate::panels::form_look::scrollbar_style),
        )
        .height(state.components_panel.list_height)
        .width(Length::Fill),
        divider,
        container(details::view(
            &state.components_panel,
            tokens,
            &ctx.canvas_colors,
            grid,
            ctx.device_px_per_logical_px,
        ))
        .height(Length::Fill)
        .width(Length::Fill),
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
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
            text(notice.message())
                .size(ROW_TEXT_SIZE)
                .color(text_c)
                .boxed(),
        );
    }
    let dismiss = button(text("Dismiss").size(ROW_TEXT_SIZE).color(text_c))
        .padding([SP.xs, SP.m])
        .on_press(LibraryMessage::IdClashNoticesDismissed)
        .style(crate::styles::menu_item(tokens));
    container(notices.push(dismiss.boxed()))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// Render a single mount-source section with its collapsible header,
/// the `+ Add Library…` action (Installed/Global only), and a
/// per-library list of components matching the filter.
fn view_section<'a>(
    src: ComponentsMountSource,
    state: &'a LibraryState,
    libs_for_source: &[&'a OpenLibrary],
    project_libs: ProjectLibraries,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let hover_c = crate::styles::ti(tokens.hover);

    let collapsed = section_collapsed(state, src);

    let count = libs_for_source.len();
    let adds_libraries = matches!(
        src,
        ComponentsMountSource::Installed | ComponentsMountSource::Global
    );
    if crate::styles::is_graphite(tokens) {
        let header = look::section_header(
            src.label(),
            count,
            collapsed,
            LibraryMessage::ComponentsPanelToggleSection(src),
            adds_libraries.then_some(LibraryMessage::ComponentsPanelAddLibrary(src)),
            tokens,
        );
        return section_body(src, state, libs_for_source, project_libs, tokens, header);
    }

    // ── Section header ───────────────────────────────────────────
    // Toggle button (chevron + label + count) on the left, optional
    // "+ Add Library…" button on the right for Installed/Global.
    let chev = signex_widgets::chevron::chevron(!collapsed, SECTION_HEADER_TEXT_SIZE, text_c);
    let header_label = format!("{}  ({count})", src.label());
    let header_btn = button(
        row![
            chev,
            Space::new().width(6),
            text(header_label)
                .size(SECTION_HEADER_TEXT_SIZE)
                .color(text_c),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6.0, SP.m])
    .width(Length::Fill)
    .on_press(LibraryMessage::ComponentsPanelToggleSection(src))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Some(Background::Color(hover_c)),
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            text_color: text_c,
            border: Border::default(),
            ..iced::widget::button::Style::default()
        }
    });

    let mut header_row = row![header_btn].spacing(0).align_y(iced::Alignment::Center);

    if adds_libraries {
        let add_btn = button(text("+ Add Library…").size(ROW_TEXT_SIZE).color(text_c))
            .padding([SP.xs, SP.m])
            .on_press(LibraryMessage::ComponentsPanelAddLibrary(src))
            .style(crate::styles::menu_item(tokens));
        header_row = header_row.push(container(add_btn).padding([0, 6]).boxed());
    }

    section_body(
        src,
        state,
        libs_for_source,
        project_libs,
        tokens,
        header_row.boxed(),
    )
}

/// `header` over the section's libraries (or its empty state) while the
/// section is expanded.
fn section_body<'a>(
    src: ComponentsMountSource,
    state: &'a LibraryState,
    libs_for_source: &[&'a OpenLibrary],
    project_libs: ProjectLibraries,
    tokens: &'a ThemeTokens,
    header: Element<'a, LibraryMessage>,
) -> Element<'a, LibraryMessage> {
    let graphite = crate::styles::is_graphite(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border_c = theme_ext::border_color(tokens);
    let section_col: Column<Element<'a, LibraryMessage>> = column![header].spacing(0);
    if section_collapsed(state, src) {
        return section_col.boxed();
    }
    if libs_for_source.is_empty() {
        let empty_msg = empty_section_text(src, project_libs);
        let line = if graphite {
            look::empty_line(empty_msg, tokens)
        } else {
            container(text(empty_msg).size(HINT_TEXT_SIZE).color(muted))
                .padding([6.0, SP.xl])
                .boxed()
        };
        return section_col.push(line).boxed();
    }
    let needle = state.components_panel.filter.trim().to_lowercase();
    let mut section_col = section_col;
    for lib in libs_for_source.iter().copied() {
        section_col = section_col.push(view_library_block(
            lib,
            &state.components_panel,
            &needle,
            tokens,
        ));
        // Graphite separates sections only; libraries share their section.
        if !graphite {
            section_col = section_col.push(thin_sep(border_c));
        }
    }
    if graphite {
        section_col = section_col.push(look::section_gap());
    }
    section_col.boxed()
}

/// Whether `src`'s section is collapsed.
fn section_collapsed(state: &LibraryState, src: ComponentsMountSource) -> bool {
    let panel = &state.components_panel;
    match src {
        ComponentsMountSource::Project => panel.collapsed_project,
        ComponentsMountSource::Installed => panel.collapsed_installed,
        ComponentsMountSource::Global => panel.collapsed_global,
        ComponentsMountSource::Stock => panel.collapsed_stock,
    }
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

/// What an empty section says. The Project section tells apart no project,
/// a project without libraries and a project whose libraries are not mounted.
fn empty_section_text(src: ComponentsMountSource, project: ProjectLibraries) -> &'static str {
    match src {
        ComponentsMountSource::Project => match project {
            ProjectLibraries::NoProject => {
                "No project libraries. Open a project to populate this section."
            }
            ProjectLibraries::NoneDeclared => {
                "This project has no libraries yet. Right-click the project, then Add New to Project > Component Library."
            }
            ProjectLibraries::DeclaredNotMounted => {
                "This project's libraries are not mounted. See the project tree."
            }
        },
        ComponentsMountSource::Installed => {
            "No session libraries. Click \"+ Add Library…\" to mount one."
        }
        ComponentsMountSource::Global => {
            "No global libraries. Click \"+ Add Library…\" or promote an installed library."
        }
        ComponentsMountSource::Stock => {
            "Stock library not found. Reinstall Signex to restore the bundled parts."
        }
    }
}

/// The left label of a component row: part number, then the MPN when set.
fn component_row_label(internal_pn: &str, mpn: &str) -> String {
    match mpn.trim() {
        "" => internal_pn.to_owned(),
        mpn => format!("{internal_pn} — {mpn}"),
    }
}

/// Render one mounted library inside its source section: a small
/// header row with the library name + a row count, then a flat list
/// of every component matching the panel-wide filter.
fn view_library_block<'a>(
    lib: &'a OpenLibrary,
    panel: &'a crate::library::state::ComponentsPanelState,
    needle: &str,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let graphite = crate::styles::is_graphite(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let muted = if graphite {
        crate::styles::tokens::resolve(tokens).ink4
    } else {
        theme_ext::text_secondary(tokens)
    };
    // Graphite selects with the neutral selected token, not the accent wash.
    let tint = if graphite {
        crate::styles::ti(tokens.selection)
    } else {
        crate::styles::tokens::selection_tint(tokens)
    };
    let hover = crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover);
    let row_look = if graphite {
        RowLook::graphite()
    } else {
        RowLook::LEGACY
    };

    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(0);

    // Library header — name + (rows) badge.
    if graphite {
        col = col.push(look::library_header(
            lib.display_name.clone(),
            lib.total_rows(),
            tokens,
        ));
    } else {
        col = col.push(
            container(
                row![
                    text(lib.display_name.clone())
                        .size(PANEL_TEXT_SIZE)
                        .color(text_c)
                        .width(Length::Fill),
                    text(format!("({})", lib.total_rows()))
                        .size(HINT_TEXT_SIZE)
                        .color(muted),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center),
            )
            .padding([SP.s, SP.xl])
            .boxed(),
        );
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
                text(format!(
                    "{} more — refine search",
                    matched - MAX_ROWS_PER_LIBRARY
                ))
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
                text("(no matches in this library)")
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
    /// `None` sizes the row to its text.
    height: Option<f32>,
    /// Left inset (also the right one of a legacy row).
    inset: f32,
    /// Right inset of a fixed-height row.
    right: f32,
    /// Face of the part number; `None` is the UI font.
    part_font: Option<iced::Font>,
}

impl RowLook {
    const LEGACY: Self = Self {
        height: None,
        inset: 28.0,
        right: 28.0,
        part_font: None,
    };

    fn graphite() -> Self {
        Self {
            height: Some(look::ROW_HEIGHT),
            inset: look::ROW_INSET,
            right: look::EDGE,
            part_font: Some(look::mono()),
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

    let mut part = text(label_left)
        .size(ROW_TEXT_SIZE)
        .color(text_c)
        .width(Length::FillPortion(3))
        .wrapping(iced::widget::text::Wrapping::None);
    if let Some(font) = row_look.part_font {
        part = part.font(font);
    }
    let content = container(
        row![
            part,
            text(label_right)
                .size(HINT_TEXT_SIZE)
                .color(muted)
                .width(Length::FillPortion(2))
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(SP.s)
        .align_y(if row_look.height.is_some() {
            iced::Alignment::Center
        } else {
            iced::Alignment::Start
        }),
    )
    .width(Length::Fill);
    let content = match row_look.height {
        Some(h) => content
            .padding(iced::Padding {
                left: row_look.inset,
                right: row_look.right,
                ..iced::Padding::ZERO
            })
            .height(h)
            .center_y(h),
        None => content.padding([SP.xs, row_look.inset]),
    };
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
mod tests {
    use super::*;
    use signex_library::{
        ComponentClass, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart, ParamMap,
        PinPadOverride, PlmReserved,
    };
    use uuid::Uuid;

    fn fixture_row(pn: &str, mpn: &str, mfr: &str) -> ComponentRow {
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
                &state.components_panel,
                "",
                tokens,
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
    fn library_with(rows: Vec<ComponentRow>) -> LibraryState {
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
                    &state.components_panel,
                    "",
                    &tokens,
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
    fn mount_source_order_is_four_sections() {
        assert_eq!(ComponentsMountSource::ORDER.len(), 4);
        assert_eq!(
            ComponentsMountSource::ORDER[3],
            ComponentsMountSource::Stock
        );
        assert_eq!(
            ComponentsMountSource::ORDER[0],
            ComponentsMountSource::Project
        );
        assert_eq!(
            ComponentsMountSource::ORDER[2],
            ComponentsMountSource::Global
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
                RowLook::LEGACY,
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
        let project = |state| empty_section_text(ComponentsMountSource::Project, state);
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
                empty_section_text(src, ProjectLibraries::NoneDeclared),
                empty_section_text(src, ProjectLibraries::NoProject)
            );
        }
    }

    #[test]
    fn a_row_without_an_mpn_drops_the_trailing_dash() {
        assert_eq!(component_row_label("STOCK-R", ""), "STOCK-R");
        assert_eq!(component_row_label("STOCK-R", "  "), "STOCK-R");
        assert_eq!(component_row_label("R0805", "RC0805"), "R0805 — RC0805");
    }
}
