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

use iced::widget::{
    Column, Space, button, column, container, row, scrollable, svg, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Theme};

use signex_library::ComponentRow;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsMountSource, LibraryState, OpenLibrary};
use crate::styles::Spacing;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: Spacing = Spacing::DEFAULT;

pub mod global_prefs;
pub mod stock;

const PANEL_TEXT_SIZE: f32 = 11.0;
const SECTION_HEADER_TEXT_SIZE: f32 = 12.0;
const ROW_TEXT_SIZE: f32 = 10.0;
/// Most rows built per library per frame; the rest are summarised by a
/// "N more — refine search" row so a huge library cannot stall the view.
const MAX_ROWS_PER_LIBRARY: usize = 200;

const SVG_CHEVRON_RIGHT: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M3 1l5 4-5 4z" fill="currentColor"/></svg>"#;
const SVG_CHEVRON_DOWN: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M1 3l4 5 4-5z" fill="currentColor"/></svg>"#;

fn chevron(open: bool) -> svg::Handle {
    if open {
        svg::Handle::from_memory(SVG_CHEVRON_DOWN)
    } else {
        svg::Handle::from_memory(SVG_CHEVRON_RIGHT)
    }
}

/// Render the Components Panel. Returns a `LibraryMessage`-typed
/// element; the dock host wraps it in `DockMessage::Library` to route
/// through the existing library dispatcher.
///
/// The Project mount-source bucket reads through
/// `ctx.projects[].libraries[].root` rather than threading a
/// separate slice. The panel context already carries the project
/// info, so the dock's `view_region` signature stays narrow.
pub fn view<'a>(
    state: &'a LibraryState,
    ctx: &'a crate::panels::PanelContext,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let border_c = theme_ext::border_color(tokens);

    // ── Header: filter input ─────────────────────────────────────
    let filter = text_input("Filter components…", &state.components_panel.filter)
        .on_input(LibraryMessage::ComponentsPanelSetFilter)
        .padding(SP.s)
        .size(PANEL_TEXT_SIZE);

    let filter_row = container(filter).padding([SP.s, 6.0]);

    // ── Three collapsible source sections ────────────────────────
    let mut col: Column<'a, LibraryMessage> =
        column![].spacing(0).width(Length::Fill).push(filter_row);

    // Derive Project library paths from the panel context. Each
    // `ProjectPanelInfo.libraries[].root` is the absolute, resolved
    // `.snxlib` path the loader handed off to the auto-mount step.
    // Bucket every mounted library up-front so view_section
    // doesn't have to borrow back into the per-call Vec
    // `project_paths` (which can't live for the panel's `'a`).
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
        col = col.push(view_section(*src, state, &bucketed[i], tokens));
        col = col.push(thin_sep(border_c));
    }

    // Empty state — every section is empty AND no libraries mounted
    if state.open_libraries.is_empty() {
        col = col.push(
            container(
                text("No libraries mounted. Use \"+ Add Library…\" or open a project.")
                    .size(PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([10.0, SP.m]),
        );
    }

    container(scrollable(col).width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Render a single mount-source section with its collapsible header,
/// the `+ Add Library…` action (Installed/Global only), and a
/// per-library list of components matching the filter.
fn view_section<'a>(
    src: ComponentsMountSource,
    state: &'a LibraryState,
    libs_for_source: &[&'a OpenLibrary],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border_c = theme_ext::border_color(tokens);
    let hover_c = crate::styles::ti(tokens.hover);

    let collapsed = match src {
        ComponentsMountSource::Project => state.components_panel.collapsed_project,
        ComponentsMountSource::Installed => state.components_panel.collapsed_installed,
        ComponentsMountSource::Global => state.components_panel.collapsed_global,
        ComponentsMountSource::Stock => state.components_panel.collapsed_stock,
    };

    let count = libs_for_source.len();

    // ── Section header ───────────────────────────────────────────
    // Toggle button (chevron + label + count) on the left, optional
    // "+ Add Library…" button on the right for Installed/Global.
    let chev = svg(chevron(!collapsed)).width(10).height(10);
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

    if matches!(
        src,
        ComponentsMountSource::Installed | ComponentsMountSource::Global
    ) {
        let add_btn = button(text("+ Add Library…").size(ROW_TEXT_SIZE).color(text_c))
            .padding([SP.xs, SP.m])
            .on_press(LibraryMessage::ComponentsPanelAddLibrary(src))
            .style(crate::styles::menu_item(tokens));
        header_row = header_row.push(container(add_btn).padding([0, 6]));
    }

    let mut section_col: Column<'a, LibraryMessage> = column![header_row].spacing(0);

    // ── Section body (only when expanded) ────────────────────────
    if !collapsed {
        if libs_for_source.is_empty() {
            let empty_msg = match src {
                ComponentsMountSource::Project => {
                    "No project libraries. Open a project to populate this section."
                }
                ComponentsMountSource::Installed => {
                    "No session libraries. Click \"+ Add Library…\" to mount one."
                }
                ComponentsMountSource::Global => {
                    "No global libraries. Click \"+ Add Library…\" or promote an installed library."
                }
                ComponentsMountSource::Stock => {
                    "Stock library not found. Reinstall Signex to restore the bundled parts."
                }
            };
            section_col = section_col.push(
                container(text(empty_msg).size(ROW_TEXT_SIZE).color(muted)).padding([6.0, SP.xl]),
            );
        } else {
            let needle = state.components_panel.filter.trim().to_lowercase();
            for lib in libs_for_source.iter().copied() {
                section_col = section_col.push(view_library_block(
                    lib,
                    &state.components_panel,
                    &needle,
                    tokens,
                ));
                section_col = section_col.push(thin_sep(border_c));
            }
        }
    }

    section_col.into()
}

/// Render one mounted library inside its source section: a small
/// header row with the library name + a row count, then a flat list
/// of every component matching the panel-wide filter. Clicking a row
/// fires `LibraryMessage::OpenComponentRow` so the existing Component
/// Preview tab opens — same flow the project tree uses.
fn view_library_block<'a>(
    lib: &'a OpenLibrary,
    panel: &'a crate::library::state::ComponentsPanelState,
    needle: &str,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let hover_c = crate::styles::ti(tokens.hover);

    let mut col: Column<'a, LibraryMessage> = column![].spacing(0);

    // Library header — name + (rows) badge.
    col = col.push(
        container(
            row![
                text(lib.display_name.clone())
                    .size(PANEL_TEXT_SIZE)
                    .color(text_c)
                    .width(Length::Fill),
                text(format!("({})", lib.total_rows()))
                    .size(ROW_TEXT_SIZE)
                    .color(muted),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, SP.xl]),
    );

    // Rows come from the cache `update` keeps current (lowercase keys and
    // the filtered list), so a frame does no per-row string work and
    // builds widgets for at most `MAX_ROWS_PER_LIBRARY` rows (#1353).
    let rows = &panel.rows;
    let matched = rows.visible_count(&lib.root);
    for (table, index) in rows.visible_rows(&lib.root, MAX_ROWS_PER_LIBRARY) {
        let Some(r) = lib.tables.get(table).and_then(|t| t.get(index)) else {
            continue;
        };
        col = col.push(view_row_button(
            lib.root.clone(),
            table.to_owned(),
            r,
            text_c,
            muted,
            hover_c,
        ));
    }

    if matched > MAX_ROWS_PER_LIBRARY {
        col = col.push(
            container(
                text(format!(
                    "{} more — refine search",
                    matched - MAX_ROWS_PER_LIBRARY
                ))
                .size(ROW_TEXT_SIZE)
                .color(muted),
            )
            .padding([SP.xs, 28.0]),
        );
    }

    if matched == 0 && !needle.is_empty() {
        col = col.push(
            container(
                text("(no matches in this library)")
                    .size(ROW_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.xs, 28.0]),
        );
    }

    col.into()
}

/// One row — the label button fires `OpenComponentRow` on click; the
/// trailing "Place" button fires `ComponentsPanelPlace` (#1160). Mirrors the
/// row layout of the existing legacy Components panel so users see
/// the same `(internal_pn — mpn)  manufacturer` shape.
fn view_row_button<'a>(
    library_path: PathBuf,
    table: String,
    row_data: &'a ComponentRow,
    text_c: Color,
    muted: Color,
    hover_c: Color,
) -> Element<'a, LibraryMessage> {
    let row_id = signex_library::RowId::from_uuid(row_data.row_id);
    let label_left = format!(
        "{} — {}",
        row_data.internal_pn.as_str(),
        row_data.primary_mpn.mpn
    );
    let label_right = row_data.primary_mpn.manufacturer.clone();
    let place = LibraryMessage::ComponentsPanelPlace {
        library_path: library_path.clone(),
        table: table.clone(),
        row_id,
    };

    let label_button = button(
        row![
            text(label_left)
                .size(ROW_TEXT_SIZE)
                .color(text_c)
                .width(Length::FillPortion(3))
                .wrapping(iced::widget::text::Wrapping::None),
            text(label_right)
                .size(ROW_TEXT_SIZE)
                .color(muted)
                .width(Length::FillPortion(2))
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(SP.s),
    )
    .padding([SP.xs, 28.0])
    .width(Length::Fill)
    .on_press(LibraryMessage::OpenComponentRow {
        library_path,
        table,
        row_id,
    })
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

    let place_button = button(text("Place").size(ROW_TEXT_SIZE).color(text_c))
        .padding([ROW_TEXT_SIZE * 0.2, ROW_TEXT_SIZE * 0.8])
        .on_press(place)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(Background::Color(hover_c)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                text_color: text_c,
                border: Border {
                    color: muted,
                    width: 1.0,
                    radius: (ROW_TEXT_SIZE * 0.3).into(),
                },
                ..iced::widget::button::Style::default()
            }
        });

    row![label_button, place_button]
        .align_y(iced::Alignment::Center)
        .spacing(ROW_TEXT_SIZE * 0.4)
        .into()
}

fn thin_sep<'a, M: 'a>(border_c: Color) -> Element<'a, M> {
    container(Space::new())
        .height(1.0)
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(border_c)),
            ..iced::widget::container::Style::default()
        })
        .into()
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
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new(pn),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::new_v4()),
            footprint_ref: None,
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
            library_id: uuid::Uuid::new_v4(),
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

        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::Signex);
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
        let places = texts.iter().filter(|t| t.text == "Place").count();
        assert_eq!(places, 200);
        crate::layout_probe::find(&texts, "50 more — refine search");

        // Narrowing the filter below the cap removes the summary row.
        state.components_panel.filter = "pn-7".into();
        state.sync_view_caches();
        let texts = crate::layout_probe::lay_out_texts(build(&state, &tokens), window);
        assert!(texts.iter().all(|t| !t.text.contains("refine search")));
        assert_eq!(texts.iter().filter(|t| t.text == "Place").count(), 11);
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

    /// Click the row's "Place" button the way the runtime would and check
    /// the message it publishes. Dropping `on_press` leaves it empty.
    #[test]
    fn clicking_a_row_place_button_publishes_the_place_message() {
        use iced::advanced::clipboard;
        use iced::{Event, Point, Size, mouse};
        use iced_runtime::UserInterface;
        use iced_runtime::user_interface::Cache;

        let row = fixture_row("R0805_10k", "ERA-3AED103V", "Panasonic");
        let row_id = signex_library::RowId::from_uuid(row.row_id);
        let window = Size::new(600.0, 200.0);
        let build = || {
            view_row_button(
                PathBuf::from("lib.snxlib"),
                "Passives".to_owned(),
                &row,
                Color::WHITE,
                Color::WHITE,
                Color::BLACK,
            )
        };

        let texts = crate::layout_probe::lay_out_texts(build(), window);
        let place = crate::layout_probe::find(&texts, "Place").bounds;
        let at = Point::new(place.x + place.width / 2.0, place.y + place.height / 2.0);

        let mut renderer = crate::layout_probe::headless_renderer();
        let mut ui = UserInterface::build(build(), window, Cache::default(), &mut renderer);
        let mut published = Vec::new();
        let _ = ui.update(
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
            mouse::Cursor::Available(at),
            &mut renderer,
            &mut clipboard::Null,
            &mut published,
        );

        assert_eq!(published.len(), 1, "one click, one message");
        match &published[0] {
            LibraryMessage::ComponentsPanelPlace {
                library_path,
                table,
                row_id: id,
            } => {
                assert_eq!(library_path, &PathBuf::from("lib.snxlib"));
                assert_eq!(table, "Passives");
                assert_eq!(*id, row_id);
            }
            _ => panic!("expected ComponentsPanelPlace"),
        }
    }

    #[test]
    fn mount_source_label_and_key_distinct() {
        for src in ComponentsMountSource::ORDER {
            assert!(!src.label().is_empty());
            assert!(!src.key().is_empty());
            assert_ne!(src.label(), src.key());
        }
    }
}
