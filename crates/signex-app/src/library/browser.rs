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
    Column, Space, button, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Border, Element, Length, Theme};
use signex_library::{ComponentRow, RowId};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::{LibraryBrowserState, LibraryState, OpenLibrary};

const BROWSER_TEXT_SIZE: f32 = 11.0;
const BROWSER_HEADER_SIZE: f32 = 10.0;
const PREVIEW_PANE_WIDTH: f32 = 380.0;
const MAX_PARAM_COLUMNS: usize = 4;

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

    // Tab strip + search header.
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
    let visible: Vec<&ComponentRow> = if needle.is_empty() {
        rows.iter().collect()
    } else {
        rows.iter()
            .filter(|r| row_matches_filter(r, &needle))
            .collect()
    };

    let columns = derive_columns(rows);

    let grid = view_grid(
        library_path,
        active_table,
        &visible,
        &columns,
        browser.selected_row,
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

    let preview_pane = view_preview_pane(library_state, &visible, browser.selected_row, tokens);

    let body = row![
        left,
        Space::new().width(8),
        container(preview_pane)
            .width(Length::Fixed(PREVIEW_PANE_WIDTH))
            .height(Length::Fill),
    ]
    .height(Length::Fill);

    column![
        header,
        container(body)
            .padding(8)
            .width(Length::Fill)
            .height(Length::Fill)
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// ─── Header (tab strip + search) ────────────────────────────────────

fn view_header<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);

    let mut tab_strip = row![].spacing(2).align_y(iced::Alignment::Center);

    let mut names: Vec<&String> = lib.tables.keys().collect();
    names.sort();

    for name in &names {
        let is_active = browser.active_table.as_deref() == Some(name.as_str());
        let label = format!(
            "{} ({})",
            name,
            lib.tables.get(*name).map(|v| v.len()).unwrap_or(0)
        );
        let library_owned = library_path.to_path_buf();
        let table_owned = (*name).clone();
        let on_press = LibraryMessage::BrowserSelectTable {
            library_path: library_owned,
            table: table_owned,
        };
        let bg_color = if is_active {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.02)
        };
        let tab_btn = button(text(label).size(BROWSER_TEXT_SIZE).color(text_c))
            .padding([4, 12])
            .on_press(on_press)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(bg_color)),
                text_color: text_c,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    color: iced::Color::TRANSPARENT,
                },
                ..iced::widget::button::Style::default()
            });
        tab_strip = tab_strip.push(tab_btn);
    }

    // The "+" button — opens the New Component modal pre-selected to
    // this library + active table.
    let library_for_plus = library_path.to_path_buf();
    let table_for_plus = browser.active_table.clone();
    let plus_btn = button(text("+").size(BROWSER_TEXT_SIZE).color(text_c))
        .padding([4, 10])
        .on_press(LibraryMessage::BrowserAddComponent {
            library_path: library_for_plus,
            table: table_for_plus,
        })
        .style(|_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: iced::Color::WHITE,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                color: iced::Color::TRANSPARENT,
            },
            ..iced::widget::button::Style::default()
        });

    let library_for_search = library_path.to_path_buf();
    let search = text_input("Search…", &browser.search)
        .on_input(move |s| LibraryMessage::BrowserSearchChanged {
            library_path: library_for_search.clone(),
            value: s,
        })
        .padding(4)
        .size(BROWSER_TEXT_SIZE)
        .width(Length::Fixed(220.0));

    container(
        row![
            tab_strip,
            Space::new().width(6),
            plus_btn,
            Space::new().width(Length::Fill),
            search,
        ]
        .spacing(0)
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 10])
    .style(crate::styles::tab_bar_strip(tokens))
    .into()
}

// ─── Grid ───────────────────────────────────────────────────────────

struct GridColumn {
    label: String,
    kind: ColumnKind,
    width: f32,
}

enum ColumnKind {
    InternalPn,
    Manufacturer,
    Mpn,
    Parameter(String),
}

/// Resolve the column list. Always: Internal PN / Manufacturer / MPN.
/// Then up to [`MAX_PARAM_COLUMNS`] of the most-common parametric keys
/// across `rows`.
fn derive_columns(rows: &[ComponentRow]) -> Vec<GridColumn> {
    let mut columns: Vec<GridColumn> = Vec::with_capacity(3 + MAX_PARAM_COLUMNS);
    columns.push(GridColumn {
        label: "Internal PN".to_string(),
        kind: ColumnKind::InternalPn,
        width: 130.0,
    });
    columns.push(GridColumn {
        label: "Manufacturer".to_string(),
        kind: ColumnKind::Manufacturer,
        width: 120.0,
    });
    columns.push(GridColumn {
        label: "MPN".to_string(),
        kind: ColumnKind::Mpn,
        width: 130.0,
    });

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
            width: 90.0,
        });
    }
    columns
}

fn shorten_label(key: &str) -> String {
    if key.len() <= 14 {
        key.to_string()
    } else {
        format!("{}…", &key[..13])
    }
}

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

fn view_grid<'a>(
    library_path: &'a std::path::Path,
    table: &str,
    rows: &[&'a ComponentRow],
    columns: &[GridColumn],
    selected: Option<RowId>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let header_row = {
        let mut r = row![].spacing(0);
        for c in columns {
            r = r.push(
                container(text(c.label.clone()).size(BROWSER_HEADER_SIZE).color(muted))
                    .padding([4, 6])
                    .width(Length::Fixed(c.width)),
            );
        }
        container(r)
            .padding([2, 4])
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                border: Border {
                    width: 0.0,
                    radius: 2.0.into(),
                    color: border,
                },
                ..Default::default()
            })
    };

    let mut col: Column<'a, LibraryMessage> = column![header_row].spacing(0).width(Length::Fill);

    if rows.is_empty() {
        col = col.push(
            container(
                text("No matching rows.")
                    .size(BROWSER_TEXT_SIZE)
                    .color(muted),
            )
            .padding([12, 6]),
        );
    } else {
        for r in rows {
            let row_id = RowId::from_uuid(r.row_id);
            let is_selected = selected == Some(row_id);
            let bg_color = if is_selected {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    0.30, 0.55, 0.85, 0.25,
                )))
            } else {
                None
            };

            let mut data_row = row![].spacing(0);
            for c in columns {
                let cell = match &c.kind {
                    // TODO(phase-2): row-double-click opens the Edit
                    // Component Details modal; cells stay text-only
                    // here in Phase 1.
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
                    container(text(cell).size(BROWSER_TEXT_SIZE).color(color))
                        .padding([2, 6])
                        .width(Length::Fixed(c.width)),
                );
            }

            let library_for_msg = library_path.to_path_buf();
            let table_for_msg = table.to_string();
            let library_for_open = library_for_msg.clone();
            let table_for_open = table_for_msg.clone();
            let row_container = container(data_row)
                .padding([0, 0])
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: bg_color,
                    border: Border {
                        width: 0.0,
                        radius: 0.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..Default::default()
                });

            // Click selects; double-click is reserved for Phase 2's
            // Edit Component Details modal — for now it routes to the
            // existing OpenComponentRow flow as a useful bridge.
            let row_widget = mouse_area(row_container)
                .on_press(LibraryMessage::BrowserSelectRow {
                    library_path: library_for_msg,
                    table: table_for_msg,
                    row_id,
                })
                .on_double_click(LibraryMessage::OpenComponentRow {
                    library_path: library_for_open,
                    table: table_for_open,
                    row_id,
                });

            col = col.push(row_widget);
        }
    }

    container(scrollable(col).width(Length::Fill).height(Length::Fill))
        .padding([0, 0])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}

// ─── Action row ─────────────────────────────────────────────────────

fn view_action_row<'a>(
    library_path: &'a std::path::Path,
    table: &str,
    selected: Option<RowId>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let library_for_add = library_path.to_path_buf();
    let table_for_add = Some(table.to_string());
    let add_btn = button(
        text("Add Component")
            .size(BROWSER_TEXT_SIZE)
            .color(iced::Color::WHITE),
    )
    .padding([4, 12])
    .on_press(LibraryMessage::BrowserAddComponent {
        library_path: library_for_add,
        table: table_for_add,
    })
    .style(|_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            0.00, 0.47, 0.84,
        ))),
        text_color: iced::Color::WHITE,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            color: iced::Color::TRANSPARENT,
        },
        ..iced::widget::button::Style::default()
    });

    let delete_btn: Element<'a, LibraryMessage> = if let Some(row_id) = selected {
        let library_for_del = library_path.to_path_buf();
        let table_for_del = table.to_string();
        button(
            text("Delete Selected")
                .size(BROWSER_TEXT_SIZE)
                .color(text_c),
        )
        .padding([4, 12])
        .on_press(LibraryMessage::BrowserDeleteRow {
            library_path: library_for_del,
            table: table_for_del,
            row_id,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        })
        .into()
    } else {
        // Greyed-out placeholder when no row is selected.
        container(text("Delete Selected").size(BROWSER_TEXT_SIZE).color(muted))
            .padding([4, 12])
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.02,
                ))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..Default::default()
            })
            .into()
    };

    container(
        row![add_btn, Space::new().width(8), delete_btn]
            .spacing(0)
            .align_y(iced::Alignment::Center),
    )
    .padding([8, 4])
    .into()
}

// ─── Preview pane ───────────────────────────────────────────────────

fn view_preview_pane<'a>(
    library_state: &'a LibraryState,
    visible: &[&'a ComponentRow],
    selected: Option<RowId>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let row_opt: Option<&ComponentRow> = selected.and_then(|id| {
        visible
            .iter()
            .find(|r| RowId::from_uuid(r.row_id) == id)
            .copied()
    });

    let body: Element<'a, LibraryMessage> = match row_opt {
        None => container(
            column![
                text("No row selected").size(13).color(text_c),
                Space::new().height(6),
                text("Click a row in the grid to preview its symbol and footprint.")
                    .size(11)
                    .color(muted),
            ]
            .spacing(0),
        )
        .padding(14)
        .center(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into(),
        Some(r) => {
            let header = container(
                column![
                    text(r.internal_pn.as_str()).size(13).color(text_c),
                    Space::new().height(2),
                    text(format!(
                        "class: {}  ·  {:?}  ·  {}",
                        r.class.as_str(),
                        r.state,
                        short_row_id(r.row_id),
                    ))
                    .size(10)
                    .color(muted),
                ]
                .spacing(0),
            )
            .padding(10);

            let symbol = library_state.set.resolve_symbol(&r.symbol_ref);
            let footprint = r
                .footprint_ref
                .as_ref()
                .and_then(|fp| library_state.set.resolve_footprint(fp));

            let symbol_panel = preview_panel("Symbol", symbol_summary(symbol.as_ref()), tokens);
            let footprint_panel =
                preview_panel("Footprint", footprint_summary(footprint.as_ref()), tokens);

            container(
                scrollable(
                    column![
                        header,
                        Space::new().height(6),
                        symbol_panel,
                        Space::new().height(8),
                        footprint_panel,
                    ]
                    .spacing(0)
                    .width(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .padding(0)
            .style(move |_: &Theme| iced::widget::container::Style {
                background: None,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..Default::default()
            })
            .into()
        }
    };

    body
}

fn short_row_id(uuid: uuid::Uuid) -> String {
    let s = uuid.simple().to_string();
    if s.len() >= 8 {
        format!("row {}", &s[..8])
    } else {
        format!("row {}", s)
    }
}

fn preview_panel<'a>(
    label: &'a str,
    summary: String,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = text(label).size(11).color(muted);
    let body = container(text(summary).size(11).color(text_c))
        .padding(10)
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..Default::default()
        });
    column![header, Space::new().height(4), body]
        .spacing(0)
        .padding([0, 10])
        .into()
}

fn symbol_summary(sym: Option<&signex_library::Symbol>) -> String {
    match sym {
        None => {
            "Symbol primitive unresolved.\n\nThe row's symbol_ref points at a UUID not currently mounted."
                .to_string()
        }
        Some(s) => {
            let pin_lines: Vec<String> = s
                .pins
                .iter()
                .take(8)
                .map(|p| format!("  · {}  {}  ({:.2}, {:.2})", p.number, p.name, p.position[0], p.position[1]))
                .collect();
            let more = if s.pins.len() > 8 {
                format!("\n  · … +{} more", s.pins.len() - 8)
            } else {
                String::new()
            };
            format!(
                "name: {}\nuuid: {}\npins: {}\n{}{}",
                s.name,
                s.uuid,
                s.pins.len(),
                pin_lines.join("\n"),
                more,
            )
        }
    }
}

fn footprint_summary(fp: Option<&signex_library::Footprint>) -> String {
    match fp {
        None => "No footprint bound.".to_string(),
        Some(f) => {
            let pad_lines: Vec<String> = f
                .pads
                .iter()
                .take(8)
                .map(|p| {
                    format!(
                        "  · pad {}  ({:.2}, {:.2}) mm",
                        p.number, p.position[0], p.position[1]
                    )
                })
                .collect();
            let more = if f.pads.len() > 8 {
                format!("\n  · … +{} more", f.pads.len() - 8)
            } else {
                String::new()
            };
            format!(
                "name: {}\nuuid: {}\npads: {}\n{}{}",
                f.name,
                f.uuid,
                f.pads.len(),
                pad_lines.join("\n"),
                more,
            )
        }
    }
}

// ─── Empty-state CTA ────────────────────────────────────────────────

fn view_empty_state<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let library_for_add = library_path.to_path_buf();
    let add_btn = button(
        text("Add Component")
            .size(BROWSER_TEXT_SIZE)
            .color(iced::Color::WHITE),
    )
    .padding([6, 14])
    .on_press(LibraryMessage::BrowserAddComponent {
        library_path: library_for_add,
        table: None,
    })
    .style(|_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            0.00, 0.47, 0.84,
        ))),
        text_color: iced::Color::WHITE,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            color: iced::Color::TRANSPARENT,
        },
        ..iced::widget::button::Style::default()
    });

    let card = container(
        column![
            text(format!("{}.snxlib", lib.display_name))
                .size(15)
                .color(text_c),
            Space::new().height(4),
            text("No categories — Add Component to begin")
                .size(12)
                .color(muted),
            Space::new().height(14),
            add_btn,
        ]
        .spacing(0)
        .align_x(iced::Alignment::Center),
    )
    .padding(28)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: border,
        },
        ..Default::default()
    });

    container(card)
        .padding(32)
        .center(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}
