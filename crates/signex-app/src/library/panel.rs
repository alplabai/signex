//! Library left-dock panel — DBLib model (v0.9-refactor-2 §10).
//!
//! Shape:
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │ [Search components…]                      │
//! ├──────────────────────────────────────────┤
//! │ ▼ MyComponents                            │  ← library node (toggleable)
//! │   ▼ Resistors  (47)                       │  ← category — inline grid expands
//! │     ┌───────────┬─────────┬──────┬────┐   │
//! │     │ Internal  │ Mfr     │ MPN  │ …  │   │
//! │     ├───────────┼─────────┼──────┼────┤   │
//! │     │ R0805_10k │ Vishay  │ CRC… │ 1% │   │
//! │     │ R0805_1k  │ Yageo   │ RC…  │ 1% │   │
//! │     └───────────┴─────────┴──────┴────┘   │
//! │   ▶ Capacitors (23)                       │
//! │ ▶ AlpLab Lib                              │
//! │ ─────────────────────────────────────────  │
//! │ [+ Open Library…]                          │
//! └──────────────────────────────────────────┘
//! ```
//!
//! Click a row → fires `LibraryMessage::OpenComponentRow { library_path,
//! table, row_id }`. Right-click on a category → "New Row in this Table".
//! Right-click on a row → "Open in Preview" / "Edit Symbol" / "Edit
//! Footprint" / "Delete Row".
//!
//! Grid columns derive from the active class template — for v0.9 we show
//! `Internal PN` / `Manufacturer` / `MPN` always, plus up to 3 of the
//! most-common parametric column keys observed across the rows in the
//! table. Class-aware column ordering can land in a follow-up patch.

use std::collections::BTreeMap;

use iced::widget::{Column, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use iced_aw::ContextMenu;
use signex_library::{ComponentRow, RowId};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::LibraryState;

const LIB_PANEL_TEXT_SIZE: f32 = 11.0;
const LIB_PANEL_HEADER_SIZE: f32 = 10.0;
const LIB_PANEL_ROW_PADDING: u16 = 4;
/// Max parametric columns we surface beyond the always-on Internal / Mfr / MPN
/// triple. Keeps the inline grid readable in narrow docks.
const MAX_PARAM_COLUMNS: usize = 3;

/// Render the Library left-dock panel.
pub fn view<'a>(state: &'a LibraryState, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let needle = state.panel_search.trim().to_lowercase();

    let search = text_input("Search components…", &state.panel_search)
        .on_input(|s| {
            // Routed through the picker filter buffer so the picker
            // modal and the panel filter share a single source of
            // truth. May split into separate buffers later if a
            // distinct UX shows up.
            LibraryMessage::Picker(super::messages::PickerMsg::FilterChanged(s))
        })
        .padding(LIB_PANEL_ROW_PADDING)
        .size(LIB_PANEL_TEXT_SIZE);

    let mut col: Column<'a, LibraryMessage> = column![]
        .spacing(2)
        .width(Length::Fill)
        .push(container(search).padding([4, 4]));

    if state.open_libraries.is_empty() {
        col = col.push(
            container(
                text("No libraries open. Use File ▸ Library ▸ Open Library… to open a *.snxlib/.")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([12, 8]),
        );
    } else {
        for (idx, lib) in state.open_libraries.iter().enumerate() {
            let expanded = state.expanded.get(idx).copied().unwrap_or(true);
            let chevron = if expanded { "▼" } else { "▶" };
            let header_label = format!("{}  {}  ({})", chevron, lib.display_name, lib.total_rows());
            let header = button(
                row![text(header_label).size(LIB_PANEL_TEXT_SIZE).color(text_c)].padding([2, 4]),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(LibraryMessage::ToggleLibraryTreeNode(idx))
            .style(crate::styles::menu_item(tokens));
            col = col.push(header);

            if !expanded {
                continue;
            }

            // Sort table names so the panel stays stable across reloads.
            let mut table_names: Vec<&String> = lib.tables.keys().collect();
            table_names.sort();

            if table_names.is_empty() {
                col = col.push(
                    container(text("(no tables)").size(LIB_PANEL_HEADER_SIZE).color(muted))
                        .padding([2, 18]),
                );
                continue;
            }

            for name in table_names {
                let rows = match lib.tables.get(name) {
                    Some(r) => r,
                    None => continue,
                };
                // Apply the search filter row by row. Empty needle ↦
                // every row passes; otherwise we accept the row when
                // its internal_pn / manufacturer / mpn matches as a
                // case-insensitive substring.
                let visible: Vec<&ComponentRow> = if needle.is_empty() {
                    rows.iter().collect()
                } else {
                    rows.iter()
                        .filter(|r| row_matches_filter(r, &needle))
                        .collect()
                };

                // When a search needle is active and the table has no
                // matches, drop the category node entirely. This keeps
                // the visible tree focused on hits.
                if !needle.is_empty() && visible.is_empty() {
                    continue;
                }

                let category_label = format!("▼  {}  ({}/{})", name, visible.len(), rows.len());

                // Category row → right-click opens "New Row in this Table".
                let category_btn = button(
                    row![text(category_label).size(LIB_PANEL_TEXT_SIZE).color(text_c),]
                        .padding([1, 4]),
                )
                .padding(0)
                .width(Length::Fill)
                // Best-effort placeholder: clicking the category row
                // is a no-op in v0.9 — the actual click target is the
                // table itself plus the right-click menu. We still
                // wire `Noop` so iced renders hover state.
                .on_press(LibraryMessage::Noop)
                .style(crate::styles::menu_item(tokens));

                let table_for_menu = name.clone();
                let category_with_menu: Element<'a, LibraryMessage> =
                    ContextMenu::new(category_btn, move || {
                        category_context_menu(&table_for_menu, tokens)
                    })
                    .into();

                col = col.push(container(category_with_menu).padding([0, 12]));

                // Inline grid — derived columns + one row per ComponentRow.
                let columns = derive_columns(rows);
                col = col.push(grid_header(&columns, tokens));
                for r in &visible {
                    col = col.push(grid_row(&lib.root, name, r, &columns, tokens));
                }
            }
        }
    }

    let footer = container(
        row![
            button(
                text("+ Open Library…")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(text_c)
            )
            .padding([4, 8])
            .on_press(LibraryMessage::OpenLibraryDialog)
            .style(crate::styles::menu_item(tokens)),
        ]
        .spacing(4),
    )
    .padding([6, 4]);

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
        scrollable(body).width(Length::Fill).height(Length::Fill),
        footer
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Case-insensitive substring match against the row's internal_pn,
/// primary manufacturer, and primary MPN.
fn row_matches_filter(r: &ComponentRow, needle: &str) -> bool {
    if r.internal_pn.as_str().to_lowercase().contains(needle) {
        return true;
    }
    if r.primary_mpn.manufacturer.to_lowercase().contains(needle) {
        return true;
    }
    if r.primary_mpn.mpn.to_lowercase().contains(needle) {
        return true;
    }
    false
}

/// One column descriptor used by the inline grid.
struct GridColumn {
    label: String,
    kind: ColumnKind,
    width: f32,
}

/// Where the column's data lives.
enum ColumnKind {
    InternalPn,
    Manufacturer,
    Mpn,
    /// Parametric — pull `r.parameters[name].display()`.
    Parameter(String),
}

/// Resolve the column list for an inline table. Always: `Internal PN`,
/// `Manufacturer`, `MPN`. Then up to [`MAX_PARAM_COLUMNS`] of the most
/// common parametric column keys across `rows`.
fn derive_columns(rows: &[ComponentRow]) -> Vec<GridColumn> {
    let mut columns: Vec<GridColumn> = Vec::with_capacity(3 + MAX_PARAM_COLUMNS);
    columns.push(GridColumn {
        label: "Internal PN".to_string(),
        kind: ColumnKind::InternalPn,
        width: 110.0,
    });
    columns.push(GridColumn {
        label: "Manufacturer".to_string(),
        kind: ColumnKind::Manufacturer,
        width: 90.0,
    });
    columns.push(GridColumn {
        label: "MPN".to_string(),
        kind: ColumnKind::Mpn,
        width: 110.0,
    });

    // Tally the parametric column frequencies so the most-used ones
    // surface first. `BTreeMap` keeps ties broken by alphabetical key
    // for a stable column order across reloads.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        for k in row.parameters.keys() {
            *counts.entry(k.clone()).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (key, _) in sorted.into_iter().take(MAX_PARAM_COLUMNS) {
        columns.push(GridColumn {
            label: shorten_label(&key),
            kind: ColumnKind::Parameter(key),
            width: 70.0,
        });
    }
    columns
}

/// Reasonable column-header truncation — the panel is narrow.
fn shorten_label(key: &str) -> String {
    if key.len() <= 12 {
        key.to_string()
    } else {
        format!("{}…", &key[..11])
    }
}

/// Render the grid header row.
fn grid_header<'a>(columns: &[GridColumn], tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let mut hdr_row = row![].spacing(0);
    for c in columns {
        hdr_row = hdr_row.push(
            container(
                text(c.label.clone())
                    .size(LIB_PANEL_HEADER_SIZE)
                    .color(muted),
            )
            .padding([2, 4])
            .width(Length::Fixed(c.width)),
        );
    }
    container(hdr_row)
        .padding([2, 24])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: None,
            border: Border {
                color: border,
                width: 0.0,
                ..Border::default()
            },
            ..Default::default()
        })
        .into()
}

/// Render one data row inside the inline grid. Wraps the row button in
/// a `ContextMenu` so a right-click surfaces the per-row menu.
fn grid_row<'a>(
    library_path: &std::path::Path,
    table: &str,
    r: &ComponentRow,
    columns: &[GridColumn],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let mut data_row = row![].spacing(0);
    for c in columns {
        let cell = match &c.kind {
            ColumnKind::InternalPn => r.internal_pn.as_str().to_string(),
            ColumnKind::Manufacturer => r.primary_mpn.manufacturer.clone(),
            ColumnKind::Mpn => r.primary_mpn.mpn.clone(),
            ColumnKind::Parameter(key) => match r.parameters.get(key) {
                Some(v) => v.display(),
                None => String::new(),
            },
        };
        let color = if matches!(c.kind, ColumnKind::InternalPn) {
            text_c
        } else {
            muted
        };
        data_row = data_row.push(
            container(text(cell).size(LIB_PANEL_TEXT_SIZE).color(color))
                .padding([1, 4])
                .width(Length::Fixed(c.width)),
        );
    }

    let row_id = RowId::from_uuid(r.row_id);
    let library_pb = library_path.to_path_buf();
    let table_owned = table.to_string();
    let on_click = LibraryMessage::OpenComponentRow {
        library_path: library_pb.clone(),
        table: table_owned.clone(),
        row_id,
    };

    let row_btn = button(data_row)
        .padding(0)
        .width(Length::Fill)
        .on_press(on_click)
        .style(crate::styles::menu_item(tokens));

    // Path resolution for the per-row right-click. Symbol/footprint
    // primitives live at `<library>/symbols/<uuid>.snxsym` and
    // `<library>/footprints/<uuid>.snxfpt` — that's the address the
    // standalone `.snxsym` / `.snxfpt` document tab consumes.
    let symbol_path = library_path
        .join("symbols")
        .join(format!("{}.snxsym", r.symbol_ref.uuid));
    let footprint_path = r.footprint_ref.map(|fp| {
        library_path
            .join("footprints")
            .join(format!("{}.snxfpt", fp.uuid))
    });
    let lib_pb_for_menu = library_pb;
    let table_for_menu = table_owned;

    let with_menu: Element<'a, LibraryMessage> =
        ContextMenu::new(container(row_btn).padding([0, 24]), move || {
            row_context_menu(
                &lib_pb_for_menu,
                &table_for_menu,
                row_id,
                &symbol_path,
                footprint_path.as_ref(),
                tokens,
            )
        })
        .into();
    with_menu
}

/// Build the right-click menu shown on a category node.
fn category_context_menu<'a>(table: &str, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let table_owned = table.to_string();
    container(
        column![menu_button(
            "New Row in this Table",
            LibraryMessage::NewComponentSetTable(table_owned),
            text_c,
            tokens,
        ),]
        .spacing(0),
    )
    .style(crate::styles::context_menu(tokens))
    .padding([4, 0])
    .width(Length::Fixed(220.0))
    .into()
}

/// Build the right-click menu shown on a data row.
fn row_context_menu<'a>(
    library_path: &std::path::Path,
    table: &str,
    row_id: RowId,
    symbol_path: &std::path::Path,
    footprint_path: Option<&std::path::PathBuf>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let library_owned = library_path.to_path_buf();
    let table_owned = table.to_string();

    let open_msg = LibraryMessage::OpenComponentRow {
        library_path: library_owned.clone(),
        table: table_owned,
        row_id,
    };
    let edit_symbol_msg = LibraryMessage::OpenPrimitiveEditor {
        path: symbol_path.to_path_buf(),
    };
    let edit_footprint_msg =
        footprint_path.map(|fp| LibraryMessage::OpenPrimitiveEditor { path: fp.clone() });

    let mut col = column![menu_button("Open in Preview", open_msg, text_c, tokens),].spacing(0);
    col = col.push(menu_button("Edit Symbol", edit_symbol_msg, text_c, tokens));
    col = match edit_footprint_msg {
        Some(msg) => col.push(menu_button("Edit Footprint", msg, text_c, tokens)),
        None => col.push(menu_button_disabled(
            "Edit Footprint (no footprint)",
            muted,
            tokens,
        )),
    };
    // Delete is a placeholder — wired through to a no-op until the
    // delete-confirm modal lands in a follow-up patch.
    col = col.push(menu_button_disabled("Delete Row (TODO)", muted, tokens));

    container(col)
        .style(crate::styles::context_menu(tokens))
        .padding([4, 0])
        .width(Length::Fixed(220.0))
        .into()
}

/// Single-line context-menu button.
fn menu_button<'a>(
    label: &str,
    on_press: LibraryMessage,
    text_color: iced::Color,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    button(
        container(text(label.to_string()).size(11).color(text_color))
            .padding([4, 12])
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(on_press)
    .style(crate::styles::menu_item(tokens))
    .into()
}

/// Greyed-out menu entry — for items that aren't wired yet. The
/// `tokens` argument is unused today but kept on the signature so the
/// disabled-item styling can be theme-aware in a follow-up patch
/// without churning every call site.
fn menu_button_disabled<'a>(
    label: &str,
    text_color: iced::Color,
    _tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(text(label.to_string()).size(11).color(text_color))
        .padding([4, 12])
        .width(Length::Fill)
        .into()
}
