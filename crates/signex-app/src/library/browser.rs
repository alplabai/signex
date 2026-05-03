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

    let columns = derive_columns(rows, lib.library_id, &library_state.template_registry, active_table);

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
    let body = row![left]
        .width(Length::Fill)
        .height(Length::Fill);

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

// ─── Header (tab strip + search) ────────────────────────────────────

/// Vertical table sidebar — replaces the old horizontal tab strip
/// with a database-style master pane: each table is one row, the
/// active one highlights, and `+ Table` (plus the inline create
/// form) anchors at the bottom. Per-tab × delete still ships with
/// the next iteration; for now an empty table is selectable and the
/// user can drop rows individually before deletion lands.
fn view_table_sidebar<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    const SIDEBAR_W: f32 = 200.0;
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let active_bg = iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10);

    let mut col = column![]
        .spacing(2)
        .padding(iced::Padding {
            top: 6.0,
            right: 0.0,
            bottom: 6.0,
            left: 0.0,
        })
        .width(Length::Fill);

    col = col.push(
        container(text("Tables").size(11).color(muted)).padding(iced::Padding {
            top: 0.0,
            right: 8.0,
            bottom: 4.0,
            left: 12.0,
        }),
    );

    // Hoist a single owned `PathBuf` out of every per-row loop.
    // Each table / class row's message constructors (`on_input` /
    // `on_press`) need an owned path inside their closure; calling
    // `library_path.to_path_buf()` per row burned 60+ heap allocs
    // per frame on a 20-table library. With this hoist the alloc
    // count is one per closure, sourced from the same `lib_pb`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let mut names: Vec<&String> = lib.tables.keys().collect();
    names.sort();
    // Surface the most recent delete-table error at the top of the
    // list so the user can see why a `×` click bounced (typically
    // "table is not empty (N rows)").
    if let Some(err) = browser.delete_error.as_ref() {
        let library_for_dismiss = lib_pb.clone();
        let dismiss = button(text("×").size(BROWSER_TEXT_SIZE).color(iced::Color::WHITE))
            .padding([0, 6])
            .on_press(LibraryMessage::BrowserDismissDeleteError {
                library_path: library_for_dismiss,
            })
            .style(|_: &Theme, _| iced::widget::button::Style {
                background: None,
                text_color: iced::Color::WHITE,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            });
        let banner = container(
            row![
                text(err.clone())
                    .size(BROWSER_TEXT_SIZE)
                    .color(iced::Color::WHITE)
                    .width(Length::Fill),
                dismiss,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .style(|_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.55, 0.18, 0.18,
            ))),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        });
        col = col.push(container(banner).padding([2, 8]));
    }

    for name in &names {
        let is_active = browser.active_table.as_deref() == Some(name.as_str());
        let count = lib.tables.get(*name).map(|v| v.len()).unwrap_or(0);
        let is_renaming = browser
            .renaming_table
            .as_ref()
            .is_some_and(|(orig, _)| orig.as_str() == name.as_str());

        // Rename mode: replace the whole row with a text input +
        // confirm/cancel buttons. Submit on Enter; Esc closes from
        // the Library Browser tab's escape handler (TODO).
        if is_renaming {
            let buffer = browser
                .renaming_table
                .as_ref()
                .map(|(_, b)| b.as_str())
                .unwrap_or("");
            let library_for_input = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let name_input = text_input("table_name", buffer)
                .on_input(move |s| LibraryMessage::BrowserSetRenameName {
                    library_path: library_for_input.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmRenameTable {
                    library_path: library_for_confirm.clone(),
                })
                .padding(4)
                .size(BROWSER_TEXT_SIZE);
            let confirm = button(text("✓").size(BROWSER_TEXT_SIZE).color(iced::Color::WHITE))
                .padding([4, 8])
                .on_press(LibraryMessage::BrowserConfirmRenameTable {
                    library_path: library_for_confirm,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb(
                        0.18, 0.36, 0.58,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                });
            let cancel = button(text("×").size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([4, 8])
                .on_press(LibraryMessage::BrowserCancelRenameTable {
                    library_path: library_for_cancel,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: None,
                    text_color: iced::Color::WHITE,
                    border: Border::default(),
                    ..iced::widget::button::Style::default()
                });
            let mut form = column![
                row![
                    name_input,
                    Space::new().width(4),
                    cancel,
                    Space::new().width(2),
                    confirm,
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(2)
            .padding([4, 8]);
            if let Some(err) = browser.rename_error.as_ref() {
                form = form.push(
                    text(err.clone())
                        .size(BROWSER_TEXT_SIZE)
                        .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
                );
            }
            col = col.push(form);
            continue;
        }

        let row_label = row![
            text((*name).clone())
                .size(BROWSER_TEXT_SIZE)
                .color(text_c)
                .width(Length::Fill),
            text(format!("{count}"))
                .size(BROWSER_TEXT_SIZE)
                .color(muted),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        let library_owned = lib_pb.clone();
        let table_owned = (*name).clone();
        let on_press = LibraryMessage::BrowserSelectTable {
            library_path: library_owned,
            table: table_owned.clone(),
        };
        let bg_color = if is_active { Some(active_bg) } else { None };
        let row_btn = button(row_label)
            .padding(iced::Padding {
                top: 5.0,
                right: 6.0,
                bottom: 5.0,
                left: 12.0,
            })
            .width(Length::Fill)
            .on_press(on_press)
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = bg_color.or_else(|| match status {
                    iced::widget::button::Status::Hovered => {
                        Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04))
                    }
                    _ => None,
                });
                iced::widget::button::Style {
                    background: bg.map(iced::Background::Color),
                    text_color: text_c,
                    border: Border {
                        width: 0.0,
                        radius: 0.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        // × delete button — sibling to the select button so click
        // routing isn't ambiguous (nested buttons confuse iced's
        // hit testing). Adapter refuses non-empty deletes; the
        // resulting Conflict error surfaces in the banner above.
        let library_for_del = lib_pb.clone();
        let table_for_del = (*name).clone();
        let delete_btn = button(text("×").size(BROWSER_TEXT_SIZE + 1.0).color(muted))
            .padding([3, 8])
            .on_press(LibraryMessage::BrowserDeleteTable {
                library_path: library_for_del,
                table: table_for_del,
            })
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let (bg, fg) = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => (
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            0.78, 0.22, 0.22, 1.0,
                        ))),
                        iced::Color::WHITE,
                    ),
                    _ => (None, muted),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: fg,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        // ✎ rename trigger — sibling to × so click routing stays
        // unambiguous. Switches the row into the inline rename
        // form above.
        let library_for_rename = lib_pb.clone();
        let table_for_rename = (*name).clone();
        let rename_btn = button(text("\u{270E}").size(BROWSER_TEXT_SIZE).color(muted))
            .padding([3, 6])
            .on_press(LibraryMessage::BrowserBeginRenameTable {
                library_path: library_for_rename,
                table: table_for_rename,
            })
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let (bg, fg) = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => (
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.10,
                        ))),
                        iced::Color::WHITE,
                    ),
                    _ => (None, muted),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: fg,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        let row_with_actions = row![row_btn, rename_btn, delete_btn,]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill);
        col = col.push(row_with_actions);
    }

    // ─── Classes section — F20 (2026-05-03) hidden ─────────────────
    // The Tables-only model collapses Class into Table. Class is
    // still stored on each `ComponentRow` (it backs the
    // `TemplateRegistry` lookup that surfaces basic-param columns),
    // but it's derived from the table name now and never edited
    // directly. The full Classes sidebar (per-library taxonomy +
    // rename/delete/add) lives behind a `false` gate so the supporting
    // message handlers in `dispatch/library.rs` stay live as dead
    // code — a follow-up cleanup pass can prune them once we're sure
    // the Tables-only model sticks.
    #[allow(clippy::overly_complex_bool_expr)]
    if false {
    col = col.push(Space::new().height(12));
    col = col.push(
        container(text("Classes").size(11).color(muted)).padding(iced::Padding {
            top: 0.0,
            right: 8.0,
            bottom: 4.0,
            left: 12.0,
        }),
    );

    if let Some(err) = browser.class_error.as_ref() {
        col = col.push(
            container(
                text(err.clone())
                    .size(BROWSER_TEXT_SIZE)
                    .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
            )
            .padding([2, 12]),
        );
    }

    let classes_list = library_state
        .set
        .get(lib.library_id)
        .map(|adapter| adapter.library_classes())
        .unwrap_or_default();

    for entry in &classes_list {
        // `if let` guards both branches in one shot — the rename
        // form draws when this row is the renaming target;
        // otherwise the static row + ✎/× buttons render below.
        // Using a bare `.unwrap()` after a separate `is_some_and`
        // check is a reliability footgun if the surrounding
        // control flow ever changes.
        if let Some((_, key_buf, label_buf)) = browser
            .renaming_class
            .as_ref()
            .filter(|(orig, _, _)| orig.as_str() == entry.key.as_str())
        {
            let library_for_key = lib_pb.clone();
            let library_for_label = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let key_input = text_input("key", key_buf)
                .on_input(move |s| LibraryMessage::BrowserSetRenameClassKey {
                    library_path: library_for_key.clone(),
                    value: s,
                })
                .padding(3)
                .size(BROWSER_TEXT_SIZE);
            let label_input = text_input("label", label_buf)
                .on_input(move |s| LibraryMessage::BrowserSetRenameClassLabel {
                    library_path: library_for_label.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmRenameClass {
                    library_path: library_for_confirm.clone(),
                })
                .padding(3)
                .size(BROWSER_TEXT_SIZE);
            let confirm = button(text("✓").size(BROWSER_TEXT_SIZE).color(iced::Color::WHITE))
                .padding([3, 6])
                .on_press(LibraryMessage::BrowserConfirmRenameClass {
                    library_path: library_for_confirm,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb(
                        0.18, 0.36, 0.58,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                });
            let cancel = button(text("×").size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([3, 6])
                .on_press(LibraryMessage::BrowserCancelRenameClass {
                    library_path: library_for_cancel,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: None,
                    text_color: iced::Color::WHITE,
                    border: Border::default(),
                    ..iced::widget::button::Style::default()
                });
            let form = column![
                key_input,
                label_input,
                row![cancel, Space::new().width(2), confirm,].align_y(iced::Alignment::Center),
            ]
            .spacing(2)
            .padding([4, 8]);
            col = col.push(form);
            continue;
        }

        let label_text = if entry.label == entry.key {
            entry.key.clone()
        } else {
            format!("{}  ·  {}", entry.label, entry.key)
        };
        let is_active_class = browser.class_filter.as_deref() == Some(entry.key.as_str());
        let library_for_filter = lib_pb.clone();
        let key_for_filter = entry.key.clone();
        let library_for_rename = lib_pb.clone();
        let library_for_delete = lib_pb.clone();
        let key_for_rename = entry.key.clone();
        let key_for_delete = entry.key.clone();
        let rename_btn = button(text("\u{270E}").size(BROWSER_TEXT_SIZE).color(muted))
            .padding([3, 6])
            .on_press(LibraryMessage::BrowserBeginRenameClass {
                library_path: library_for_rename,
                key: key_for_rename,
            })
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let (bg, fg) = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => (
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.10,
                        ))),
                        iced::Color::WHITE,
                    ),
                    _ => (None, muted),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: fg,
                    border: Border {
                        radius: 2.0.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });
        let delete_btn = button(text("×").size(BROWSER_TEXT_SIZE + 1.0).color(muted))
            .padding([3, 8])
            .on_press(LibraryMessage::BrowserDeleteClass {
                library_path: library_for_delete,
                key: key_for_delete,
            })
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let (bg, fg) = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => (
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            0.78, 0.22, 0.22, 1.0,
                        ))),
                        iced::Color::WHITE,
                    ),
                    _ => (None, muted),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: fg,
                    border: Border {
                        radius: 2.0.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });
        let class_active_bg = active_bg;
        let class_btn = button(
            text(label_text)
                .size(BROWSER_TEXT_SIZE)
                .color(text_c)
                .width(Length::Fill),
        )
        .padding(iced::Padding {
            top: 4.0,
            right: 6.0,
            bottom: 4.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .on_press(LibraryMessage::BrowserClassFilterClicked {
            library_path: library_for_filter,
            key: key_for_filter,
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = if is_active_class {
                Some(class_active_bg)
            } else {
                match status {
                    iced::widget::button::Status::Hovered => {
                        Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04))
                    }
                    _ => None,
                }
            };
            iced::widget::button::Style {
                background: bg.map(iced::Background::Color),
                text_color: text_c,
                border: Border {
                    width: 0.0,
                    radius: 0.0.into(),
                    color: iced::Color::TRANSPARENT,
                },
                ..iced::widget::button::Style::default()
            }
        });
        let class_row = row![class_btn, rename_btn, delete_btn,]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill);
        col = col.push(class_row);
    }

    // + Class form / button.
    match browser.adding_class.as_ref() {
        Some(draft) => {
            let library_for_key = lib_pb.clone();
            let library_for_label = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let key_input = text_input("class_key", &draft.key)
                .on_input(move |s| LibraryMessage::BrowserSetNewClassKey {
                    library_path: library_for_key.clone(),
                    value: s,
                })
                .padding(3)
                .size(BROWSER_TEXT_SIZE);
            let label_input = text_input("Label", &draft.label)
                .on_input(move |s| LibraryMessage::BrowserSetNewClassLabel {
                    library_path: library_for_label.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmAddClass {
                    library_path: library_for_confirm.clone(),
                })
                .padding(3)
                .size(BROWSER_TEXT_SIZE);
            let confirm = button(
                text("Create")
                    .size(BROWSER_TEXT_SIZE)
                    .color(iced::Color::WHITE),
            )
            .padding([3, 8])
            .on_press(LibraryMessage::BrowserConfirmAddClass {
                library_path: library_for_confirm,
            })
            .style(|_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.18, 0.36, 0.58,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            });
            let cancel = button(text("Cancel").size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([3, 8])
                .on_press(LibraryMessage::BrowserCancelAddClass {
                    library_path: library_for_cancel,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: Border {
                        radius: 2.0.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                });
            let mut form = column![
                key_input,
                label_input,
                row![cancel, Space::new().width(4), confirm,].align_y(iced::Alignment::Center),
            ]
            .spacing(2)
            .padding([4, 8]);
            if let Some(err) = draft.error.as_ref() {
                form = form.push(
                    text(err.clone())
                        .size(BROWSER_TEXT_SIZE)
                        .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
                );
            }
            col = col.push(form);
        }
        None => {
            let library_for_begin = lib_pb.clone();
            col = col.push(
                container(
                    button(text("+ Class").size(BROWSER_TEXT_SIZE).color(text_c))
                        .padding([4, 10])
                        .width(Length::Fill)
                        .on_press(LibraryMessage::BrowserBeginAddClass {
                            library_path: library_for_begin,
                        })
                        .style(|_: &Theme, _| iced::widget::button::Style {
                            background: Some(iced::Background::Color(iced::Color::from_rgba(
                                1.0, 1.0, 1.0, 0.04,
                            ))),
                            text_color: iced::Color::WHITE,
                            border: Border {
                                radius: 3.0.into(),
                                ..Border::default()
                            },
                            ..iced::widget::button::Style::default()
                        }),
                )
                .padding(iced::Padding {
                    top: 4.0,
                    right: 8.0,
                    bottom: 6.0,
                    left: 8.0,
                }),
            );
        }
    }
    } // end `if false` — Classes section gate (F20)

    // Inline `+ Table` form / button — same lifecycle as before, now
    // anchored at the bottom of the sidebar.
    let bottom_section: Element<'a, LibraryMessage> = match browser.adding_table.as_ref() {
        Some(draft) => {
            let library_for_input = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let name_input = text_input("new_table_name", &draft.name)
                .on_input(move |s| LibraryMessage::BrowserSetNewTableName {
                    library_path: library_for_input.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmAddTable {
                    library_path: library_for_confirm.clone(),
                })
                .padding(4)
                .size(BROWSER_TEXT_SIZE);
            let confirm = button(
                text("Create")
                    .size(BROWSER_TEXT_SIZE)
                    .color(iced::Color::WHITE),
            )
            .padding([4, 10])
            .on_press(LibraryMessage::BrowserConfirmAddTable {
                library_path: library_for_confirm,
            })
            .style(|_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.18, 0.36, 0.58,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    color: iced::Color::TRANSPARENT,
                },
                ..iced::widget::button::Style::default()
            });
            let cancel = button(text("Cancel").size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([4, 10])
                .on_press(LibraryMessage::BrowserCancelAddTable {
                    library_path: library_for_cancel,
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
            let mut form = column![
                name_input,
                row![cancel, Space::new().width(4), confirm,].align_y(iced::Alignment::Center),
            ]
            .spacing(4)
            .padding([6, 12]);
            if let Some(err) = draft.error.as_ref() {
                form = form.push(
                    text(err.clone())
                        .size(BROWSER_TEXT_SIZE)
                        .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
                );
            }
            form.into()
        }
        None => {
            let library_for_begin = lib_pb.clone();
            container(
                button(text("+ Table").size(BROWSER_TEXT_SIZE).color(text_c))
                    .padding([4, 10])
                    .width(Length::Fill)
                    .on_press(LibraryMessage::BrowserBeginAddTable {
                        library_path: library_for_begin,
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
                    }),
            )
            .padding(iced::Padding {
                top: 4.0,
                right: 8.0,
                bottom: 6.0,
                left: 8.0,
            })
            .into()
        }
    };

    col = col.push(Space::new().height(Length::Fill));
    col = col.push(bottom_section);

    container(col)
        .width(Length::Fixed(SIDEBAR_W))
        .height(Length::Fill)
        .into()
}

fn view_header<'a>(
    library_path: &'a std::path::Path,
    _lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    // The "+" button — opens the New Component modal pre-selected to
    // this library + active table.
    let library_for_plus = lib_pb.clone();
    let table_for_plus = browser.active_table.clone();
    let plus_btn = button(text("+ Component").size(BROWSER_TEXT_SIZE).color(text_c))
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

    // Add-table control. Either the inline name form (while
    let library_for_search = lib_pb.clone();
    let search = text_input("Search…", &browser.search)
        .on_input(move |s| LibraryMessage::BrowserSearchChanged {
            library_path: library_for_search.clone(),
            value: s,
        })
        .padding(4)
        .size(BROWSER_TEXT_SIZE)
        .width(Length::Fixed(220.0));

    // Lifecycle filter pill — Stage 18 surfaces `ComponentRow.state`
    // as a first-class browser filter so users can pivot between
    // "preferred only", "include deprecated", etc. without touching
    // every row's lifecycle field.
    let library_for_lc = lib_pb.clone();
    let lifecycle_picker = pick_list(
        LifecycleFilter::ALL.to_vec(),
        Some(browser.lifecycle_filter),
        move |f| LibraryMessage::BrowserSetLifecycleFilter {
            library_path: library_for_lc.clone(),
            filter: f,
        },
    )
    .placeholder("Lifecycle")
    .padding(4)
    .text_size(BROWSER_TEXT_SIZE);

    container(
        row![
            plus_btn,
            Space::new().width(Length::Fill),
            lifecycle_picker,
            Space::new().width(8),
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
    /// Stage 14 — combined version + released indicator. Renders as
    /// `🔒 1.2.3` for released rows, `1.2.3` for unreleased. Sorts
    /// against the row's `version` cell so semver patterns sort
    /// numerically (1.0.2 vs 1.0.10 — `compare_cells` handles the
    /// pure-number case; mixed `1.0.2` strings fall back to lexical
    /// which still works for short major.minor.patch strings).
    Rev,
    /// Read-only column showing the row's bound symbol primitive.
    /// Empty cell (`—`) when the row's `symbol_ref` is `Uuid::nil()`
    /// (the sentinel for an unbound row). The actual binding edit
    /// surface is the Properties panel's "Pick Symbol…" button — the
    /// column gives an at-a-glance status across the whole table.
    /// F16 of the 2026-05-03 library polish ("the relevant columns
    /// must be there by default").
    Symbol,
    /// Read-only column showing the row's bound footprint primitive.
    /// Empty cell (`—`) when `footprint_ref` is `None` or its UUID is
    /// `Uuid::nil()`. Edited via Properties panel "Pick Footprint…".
    Footprint,
    /// Stage 18 — read-only column reading from `parameters["tags"]`.
    /// Inline-editable through the leftmost cell-edit buffer pattern
    /// is deferred to a polish pass; for now the canonical edit point
    /// is the Edit Component Details modal.
    Tags,
    Parameter(String),
}

impl ColumnKind {
    /// Stable sort key matching `LibraryMessage::BrowserSortColumn`'s
    /// `column_key` field. Ties columns to their cell-edit buffers
    /// and to the [`super::state::BrowserSort`] state.
    fn sort_key(&self) -> String {
        match self {
            ColumnKind::InternalPn => "internal_pn".to_string(),
            ColumnKind::Manufacturer => "manufacturer".to_string(),
            ColumnKind::Mpn => "mpn".to_string(),
            ColumnKind::Rev => "version".to_string(),
            ColumnKind::Symbol => "symbol_ref".to_string(),
            ColumnKind::Footprint => "footprint_ref".to_string(),
            ColumnKind::Tags => "parameters.tags".to_string(),
            ColumnKind::Parameter(key) => format!("parameters.{key}"),
        }
    }

    /// Extract the row's cell value for this column. Empty string when
    /// the row has no value for the underlying field. For
    /// [`ColumnKind::Rev`], the value is the bare semver string (the
    /// 🔒/🔓 badge is added at render time, not in the sort key, so
    /// released and unreleased rows still sort by version order).
    fn cell_value(&self, r: &ComponentRow) -> String {
        match self {
            ColumnKind::InternalPn => r.internal_pn.as_str().to_string(),
            ColumnKind::Manufacturer => r.primary_mpn.manufacturer.clone(),
            ColumnKind::Mpn => r.primary_mpn.mpn.clone(),
            ColumnKind::Rev => r.version.clone(),
            ColumnKind::Symbol => {
                if r.symbol_ref.uuid == uuid::Uuid::nil() {
                    "—".to_string()
                } else {
                    // Surface the short uuid prefix so the user has an
                    // at-a-glance signal without bloating the column
                    // width. Full path/name is in the Properties panel.
                    format!("• {:.8}", r.symbol_ref.uuid)
                }
            }
            ColumnKind::Footprint => match &r.footprint_ref {
                Some(fp) if fp.uuid != uuid::Uuid::nil() => {
                    format!("• {:.8}", fp.uuid)
                }
                _ => "—".to_string(),
            },
            ColumnKind::Tags => match r.parameters.get("tags") {
                Some(v) => v.display(),
                None => String::new(),
            },
            ColumnKind::Parameter(key) => match r.parameters.get(key) {
                Some(v) => v.display(),
                None => String::new(),
            },
        }
    }
}

/// Comparator for two cell strings with auto-detected numeric
/// fallback. If both values parse as `f64`, sort numerically;
/// otherwise sort case-insensitively. This is Stage 8's answer to
/// the Altium "lexical sort on numeric columns" pain — we don't
/// need a typed schema lookup at compare time, and untyped legacy
/// columns get the right behaviour automatically when their cells
/// happen to be numeric.
fn compare_cells(a: &str, b: &str) -> std::cmp::Ordering {
    if let (Ok(na), Ok(nb)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        return na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal);
    }
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Resolve the column list. Always: Internal PN / Manufacturer / MPN /
/// Rev / Symbol / Footprint. Then template-derived columns from the
/// `TemplateRegistry`: every `required_param` slot from the templates
/// resolved for the table's classes (de-duplicated across classes).
/// Then a Tags column (Stage 18) when *any* row carries a non-empty
/// `parameters["tags"]`. Finally up to [`MAX_PARAM_COLUMNS`] of the
/// most-common other parametric keys across `rows` — `tags` and any
/// already-shown template params are excluded so columns don't render
/// twice.
///
/// Template resolution sources `class` from the rows when present;
/// for an empty table it strips a trailing "s" off `table_name` and
/// uses that as the implicit class (works for the default
/// pluralisation `resistor` → `resistors` etc.). F19 / F20 of the
/// 2026-05-03 library polish: the user wanted basic params per table
/// to appear by default, AND they want Tables to be the only
/// user-facing concept (Classes are now derived purely from the
/// table name, never edited directly).
fn derive_columns(
    rows: &[ComponentRow],
    library_id: uuid::Uuid,
    registry: &signex_library::TemplateRegistry,
    table_name: &str,
) -> Vec<GridColumn> {
    let mut columns: Vec<GridColumn> = Vec::with_capacity(4 + MAX_PARAM_COLUMNS);
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

    // Stage 14: surface row revision + released-flag as a single
    // column. Rendered with a 🔒 prefix on released rows. Always
    // present so the user has a stable place to spot drift even
    // when no rows are flagged released yet.
    columns.push(GridColumn {
        label: "Rev".to_string(),
        kind: ColumnKind::Rev,
        width: 80.0,
    });

    // F16 (2026-05-03 library polish) — Symbol + Footprint binding
    // status is shown by default. `—` means unbound; short uuid
    // prefix means bound. Full primitive path/name + the Pick…
    // affordance live in the Properties panel for the selected row.
    columns.push(GridColumn {
        label: "Symbol".to_string(),
        kind: ColumnKind::Symbol,
        width: 120.0,
    });
    columns.push(GridColumn {
        label: "Footprint".to_string(),
        kind: ColumnKind::Footprint,
        width: 120.0,
    });

    // F19 — template-derived basic-parameter columns. Resolve unique
    // classes from the rows; for empty tables, strip a trailing "s"
    // off the table name to derive an implicit class (works for the
    // default pluralisation, falls through harmlessly otherwise).
    // Each template's `required_params` becomes a column with
    // "<name> (<unit>)" label so users see the canonical units up
    // front. Already-added param keys are skipped to dedupe across
    // classes.
    let mut classes: std::collections::BTreeSet<String> =
        rows.iter().map(|r| r.class.as_str().to_string()).collect();
    if classes.is_empty() {
        if let Some(stem) = table_name.strip_suffix('s') {
            classes.insert(stem.to_string());
        }
    }
    for class in &classes {
        if let Some(tmpl) = registry.resolve(library_id, class) {
            for slot in &tmpl.required_params {
                let already = columns.iter().any(|c| {
                    matches!(&c.kind, ColumnKind::Parameter(k) if k == &slot.name)
                });
                if already {
                    continue;
                }
                // Label is the slot name only — no `(unit)` suffix.
                // Units vary per row (a "value" column holds 10kΩ in
                // resistors, 4.7µF in capacitors), so the column
                // header must be unit-agnostic; the cell renders the
                // unit inline via `ParamValue::Measurement.display()`.
                // Capitalises the first letter so "value" → "Value" /
                // "tolerance" → "Tolerance" without bringing a
                // heavy-weight casing crate in.
                let label = {
                    let mut chars = slot.name.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                };
                columns.push(GridColumn {
                    label,
                    kind: ColumnKind::Parameter(slot.name.clone()),
                    width: 110.0,
                });
            }
        }
    }

    // Surface tags as a first-class column whenever the table has at
    // least one tagged row. Saves the user from having to scroll
    // sideways through the auto-derived parameter columns to find them.
    let any_tagged = rows
        .iter()
        .any(|r| matches!(r.parameters.get("tags"), Some(v) if !v.display().is_empty()));
    if any_tagged {
        columns.push(GridColumn {
            label: "Tags".to_string(),
            kind: ColumnKind::Tags,
            width: 160.0,
        });
    }

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        for k in row.parameters.keys() {
            if k == "tags" {
                // Already surfaced via the dedicated Tags column.
                continue;
            }
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

/// Per-lifecycle indicator dot colour. Matches plan §6:
///
/// * Released → green;
/// * Draft / InReview → neutral grey ("active, but not preferred");
/// * Deprecated → amber/yellow;
/// * Obsolete → muted dark grey.
///
/// Centralised here so both the dot and any future lifecycle badge
/// in the side preview pane can pull the same colour.
fn lifecycle_dot_color(state: LifecycleState) -> iced::Color {
    match state {
        LifecycleState::Released => iced::Color::from_rgb(0.30, 0.78, 0.40),
        LifecycleState::InReview => iced::Color::from_rgb(0.50, 0.65, 0.95),
        LifecycleState::Draft => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.45),
        LifecycleState::Deprecated => iced::Color::from_rgb(0.96, 0.78, 0.10),
        LifecycleState::Obsolete => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.20),
        // `LifecycleState` is `#[non_exhaustive]` — fall back to the
        // muted "Draft" colour for any future state we haven't styled yet.
        _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.30),
    }
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
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let _text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let selected = browser.selected_row;
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let header_row = {
        // 16-px gutter aligns with the per-row lifecycle dot below.
        let mut r = row![Space::new().width(Length::Fixed(LIFECYCLE_DOT_GUTTER))].spacing(0);
        let active_sort = browser.sort_by.as_ref();
        for c in columns {
            // Indicate the active sort column with a "▲" / "▼" glyph
            // appended to the label. Other columns render plain so the
            // user can spot the sort target at a glance.
            let column_key = c.kind.sort_key();
            let label_text = match active_sort {
                Some(s) if s.key == column_key => {
                    let arrow = if s.descending { "▼" } else { "▲" };
                    format!("{}  {arrow}", c.label)
                }
                _ => c.label.clone(),
            };
            // Wrap the header label in a borderless button so a click
            // toggles the sort. Stage 8 of `v0.9-snxlib-as-file-plan.md`.
            let library_for_sort = lib_pb.clone();
            let header_btn = button(text(label_text).size(BROWSER_HEADER_SIZE).color(muted))
                .padding([4, 6])
                .on_press(LibraryMessage::BrowserSortColumn {
                    library_path: library_for_sort,
                    column_key,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: None,
                    text_color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.65),
                    border: Border {
                        width: 0.0,
                        radius: 0.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                });
            r = r.push(container(header_btn).width(Length::Fixed(c.width)));
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
            // Lifecycle tinting — deprecated rows render with a faint
            // amber wash so the user spots them at a glance even when
            // they're shown alongside released rows under the
            // `IncludeDeprecated` filter (plan §6).
            let lifecycle_tint = matches!(r.state, LifecycleState::Deprecated)
                .then(|| iced::Background::Color(iced::Color::from_rgba(0.96, 0.80, 0.10, 0.10)));
            let bg_color = if is_selected {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    0.30, 0.55, 0.85, 0.25,
                )))
            } else {
                lifecycle_tint
            };

            // Lifecycle indicator dot — Stage 18. Sits in the
            // 16-px gutter mirrored on the header row.
            let dot_color = lifecycle_dot_color(r.state);
            let lifecycle_dot = container(Space::new())
                .width(Length::Fixed(LIFECYCLE_DOT_SIZE))
                .height(Length::Fixed(LIFECYCLE_DOT_SIZE))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(dot_color)),
                    border: Border {
                        width: 0.0,
                        radius: (LIFECYCLE_DOT_SIZE / 2.0).into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..Default::default()
                });
            let lifecycle_cell = container(lifecycle_dot)
                .width(Length::Fixed(LIFECYCLE_DOT_GUTTER))
                .height(Length::Fill)
                .center_y(Length::Fill)
                .padding([0, 4]);

            let mut data_row = row![lifecycle_cell].spacing(0);
            for c in columns {
                let column_key = c.kind.sort_key();
                let row_value = c.kind.cell_value(r);

                // Rev column: read-only, prefixes a 🔒 glyph when
                // released. Stage 14 of v0.9-snxlib-as-file. Edit
                // path is the bump dialog (Team mode) or auto-bump
                // on save (Personal mode), not inline grid editing.
                if matches!(c.kind, ColumnKind::Rev) {
                    let label = if r.released {
                        format!("🔒 {row_value}")
                    } else {
                        row_value.clone()
                    };
                    let color = if r.released {
                        // Released rows lock-icon-amber so the gate
                        // stands out at a glance.
                        iced::Color::from_rgb(0.96, 0.78, 0.10)
                    } else {
                        theme_ext::text_secondary(tokens)
                    };
                    data_row = data_row.push(
                        container(text(label).size(BROWSER_TEXT_SIZE).color(color))
                            .padding([4, 6])
                            .width(Length::Fixed(c.width)),
                    );
                    continue;
                }

                // Tags render read-only for now — canonical edit point
                // is the Edit Component Details modal (plan §6 calls
                // tag editing a modal-only flow until a chip-input
                // widget lands).
                if matches!(c.kind, ColumnKind::Tags) {
                    data_row = data_row.push(
                        container(
                            text(row_value)
                                .size(BROWSER_TEXT_SIZE)
                                .color(theme_ext::text_secondary(tokens)),
                        )
                        .padding([4, 6])
                        .width(Length::Fixed(c.width)),
                    );
                    continue;
                }

                // Symbol + Footprint cells are read-only status
                // markers (`—` unbound, `• <uuid8>` bound). Binding
                // edit happens in the Properties panel via Pick
                // Symbol / Pick Footprint — typing into the cell
                // would have no defined semantics.
                if matches!(c.kind, ColumnKind::Symbol | ColumnKind::Footprint) {
                    data_row = data_row.push(
                        container(
                            text(row_value)
                                .size(BROWSER_TEXT_SIZE)
                                .color(theme_ext::text_secondary(tokens)),
                        )
                        .padding([4, 6])
                        .width(Length::Fixed(c.width)),
                    );
                    continue;
                }

                // Buffer wins over row when active.
                let buf_key = (row_id, column_key.clone());
                let cell_value = browser
                    .cell_edit
                    .get(&buf_key)
                    .cloned()
                    .unwrap_or(row_value);

                let library_for_input = lib_pb.clone();
                let column_for_input = column_key.clone();
                let library_for_submit = lib_pb.clone();
                let table_for_submit = table.to_string();
                let column_for_submit = column_key.clone();
                let input = text_input("", &cell_value)
                    .on_input(move |s| LibraryMessage::BrowserCellEdit {
                        library_path: library_for_input.clone(),
                        row_id,
                        column: column_for_input.clone(),
                        value: s,
                    })
                    .on_submit(LibraryMessage::BrowserCellCommit {
                        library_path: library_for_submit,
                        table: table_for_submit,
                        row_id,
                        column: column_for_submit,
                    })
                    .padding([2, 6])
                    .size(BROWSER_TEXT_SIZE);
                data_row = data_row.push(
                    container(input)
                        .padding([2, 2])
                        .width(Length::Fixed(c.width)),
                );
            }

            let library_for_msg = lib_pb.clone();
            let table_for_msg = table.to_string();
            let library_for_open = library_for_msg.clone();
            let table_for_open = table_for_msg.clone();
            let library_for_refresh = library_for_msg.clone();
            let table_for_refresh = table_for_msg.clone();
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

            // Click selects; double-click opens the Edit Component
            // Details modal (Deliverable B). The text_inputs above
            // capture single-clicks for focus, so the row's
            // on_press/on_double_click only fires on the gaps between
            // cells — that's fine for selection because the user is
            // clicking on a row, not a specific cell.
            //
            // Right-press fires the Stage 18 distributor refresh stub
            // — tonight's wiring just emits a tracing log; the real
            // adapter call lands when the row-binding loop is built.
            let row_widget = mouse_area(row_container)
                .on_press(LibraryMessage::BrowserSelectRow {
                    library_path: library_for_msg,
                    table: table_for_msg,
                    row_id,
                })
                .on_double_click(LibraryMessage::BrowserOpenEditModal {
                    library_path: library_for_open,
                    table: table_for_open,
                    row_id,
                })
                .on_right_press(LibraryMessage::BrowserRefreshPricing {
                    library_path: library_for_refresh,
                    table: table_for_refresh,
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
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_add = lib_pb.clone();
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
        let library_for_del = lib_pb.clone();
        let table_for_del = table.to_string();
        button(
            text("Delete Selected")
                .size(BROWSER_TEXT_SIZE)
                .color(text_c),
        )
        .padding([4, 12])
        .on_press(LibraryMessage::BrowserDeleteRowRequest {
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

// ─── Preview pane (DEAD: F15 final pass moved this to Properties) ──
//
// view_preview_pane / preview_panel / preview_panel_with_pick /
// symbol_summary / footprint_summary / short_row_id are kept as
// dead code so the prune is reviewable in one commit. The Properties
// panel reads `PanelContext.library_row_detail` and renders the
// equivalent. Pruning this block in the next cleanup pass.

#[allow(dead_code)]
fn view_preview_pane<'a>(
    library_path: &'a std::path::Path,
    table: &str,
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

            // F15 — bind primitives directly from the inline preview.
            // BrowserRow target applies + saves through the adapter
            // without needing a Component Preview tab open.
            let row_id = RowId::from_uuid(r.row_id);
            let address = crate::library::state::EditorAddress::new(
                library_path.to_path_buf(),
                table.to_string(),
                row_id,
            );

            let symbol_panel = preview_panel_with_pick(
                "Symbol",
                symbol_summary(symbol.as_ref()),
                "Pick Symbol…",
                LibraryMessage::OpenPrimitivePicker {
                    kind: signex_library::PrimitiveKind::Symbol,
                    target: crate::library::state::PrimitivePickerTarget::BrowserRow(
                        address.clone(),
                    ),
                },
                tokens,
            );
            let footprint_panel = preview_panel_with_pick(
                "Footprint",
                footprint_summary(footprint.as_ref()),
                "Pick Footprint…",
                LibraryMessage::OpenPrimitivePicker {
                    kind: signex_library::PrimitiveKind::Footprint,
                    target: crate::library::state::PrimitivePickerTarget::BrowserRow(address),
                },
                tokens,
            );

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

#[allow(dead_code)]
fn short_row_id(uuid: uuid::Uuid) -> String {
    let s = uuid.simple().to_string();
    if s.len() >= 8 {
        format!("row {}", &s[..8])
    } else {
        format!("row {}", s)
    }
}

#[allow(dead_code)]
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

/// Same as [`preview_panel`] but adds a Pick… button on the right
/// of the header row. F15 — primitive binding lives next to the row
/// status so the user has the Pick button visible whenever they see
/// "unbound" or "unresolved".
#[allow(dead_code)]
fn preview_panel_with_pick<'a>(
    label: &'a str,
    summary: String,
    pick_label: &'a str,
    pick_msg: LibraryMessage,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let pick_btn = button(text(pick_label).size(10).color(text_c))
        .padding([3, 8])
        .on_press(pick_msg)
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
        });

    let header = row![
        text(label).size(11).color(muted),
        Space::new().width(Length::Fill),
        pick_btn,
    ]
    .align_y(iced::Alignment::Center);

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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_add = lib_pb.clone();
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
