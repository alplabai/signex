//! Library Browser — the data grid view.
//!
//! Header row (click-to-sort) plus one editable/read-only row per
//! visible `ComponentRow`, with the per-row lifecycle dot + tint.
//! Extracted verbatim from the former single-file `browser` module.

use super::columns::{ColumnKind, GridColumn, lifecycle_dot_color};
use super::keyed::keyed;
use super::size_sensor::sense_height;
use super::window::{
    BROWSER_HEADER_HEIGHT, BROWSER_ROW_HEIGHT, grid_scroll_id, library_key, visible_rows,
};
use super::*;
use crate::library::dangling::DanglingSweep;
use iced::widget::column;
use signex_library::RefSlot;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The read-only caches the grid cells draw from.
pub(super) struct GridCaches<'a> {
    pub(super) dangling: &'a DanglingSweep,
    pub(super) prices: &'a super::price::PriceView<'a>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "one grid view over the browser, table and theme state"
)]
pub(super) fn view_grid<'a>(
    library_path: &'a std::path::Path,
    table: &str,
    rows: &[&'a ComponentRow],
    columns: &[GridColumn],
    browser: &'a LibraryBrowserState,
    caches: GridCaches<'_>,
    tokens: &'a ThemeTokens,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let GridCaches { dangling, prices } = caches;
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
            let header_btn = button(
                text(label_text)
                    .size(BROWSER_HEADER_SIZE)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .color(muted),
            )
            .padding([SP.s, 6.0])
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
            .padding([SP.xs, SP.s])
            .height(Length::Fixed(BROWSER_HEADER_HEIGHT))
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
                    .wrapping(iced::widget::text::Wrapping::None)
                    .color(muted),
            )
            .padding([SP.l, 6.0]),
        );
    } else {
        // Window: only the rows near the viewport get widgets; spacers of
        // the exact skipped height stand in for the rest (#1355).
        let scroll = browser.grid_scroll;
        let window = visible_rows(scroll, rows.len());
        if window.start > 0 {
            col = col
                .push(Space::new().height(Length::Fixed(window.start as f32 * BROWSER_ROW_HEIGHT)));
        }
        for r in &rows[window.clone()] {
            #[cfg(test)]
            super::window::counters::ROWS_BUILT.with(|c| c.set(c.get() + 1));
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
                .padding([0.0, SP.s]);

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
                        container(
                            text(label)
                                .size(BROWSER_TEXT_SIZE)
                                .wrapping(iced::widget::text::Wrapping::None)
                                .color(color),
                        )
                        .padding([SP.s, 6.0])
                        .width(Length::Fixed(c.width)),
                    );
                    continue;
                }

                // Cached price: read-only; Refresh All Pricing is the only
                // writer.
                if matches!(c.kind, ColumnKind::Price) {
                    data_row = data_row.push(
                        container(
                            text(prices.text(r))
                                .size(BROWSER_TEXT_SIZE)
                                .wrapping(iced::widget::text::Wrapping::None)
                                .color(theme_ext::text_secondary(tokens)),
                        )
                        .padding([SP.s, 6.0])
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
                                .wrapping(iced::widget::text::Wrapping::None)
                                .color(theme_ext::text_secondary(tokens)),
                        )
                        .padding([SP.s, 6.0])
                        .width(Length::Fixed(c.width)),
                    );
                    continue;
                }

                // Symbol + Footprint cells are read-only status
                // markers (`• <uuid8>` bound). An unbound cell reads
                // "No symbol bound" / "No footprint bound" in the
                // warning colour and is itself the Pick Symbol /
                // Pick Footprint button.
                if matches!(c.kind, ColumnKind::Symbol | ColumnKind::Footprint) {
                    let is_symbol = matches!(c.kind, ColumnKind::Symbol);
                    let unbound = if is_symbol {
                        r.is_symbol_unbound()
                    } else {
                        r.is_footprint_unbound()
                    };
                    if unbound {
                        let (kind, label) = if is_symbol {
                            (signex_library::PrimitiveKind::Symbol, "No symbol bound")
                        } else {
                            (
                                signex_library::PrimitiveKind::Footprint,
                                "No footprint bound",
                            )
                        };
                        let address = crate::library::state::EditorAddress::new(
                            lib_pb.clone(),
                            table.to_string(),
                            row_id,
                        );
                        data_row = data_row.push(
                            container(unbound_pick_button(label, kind, address, tokens))
                                .padding([SP.xs, SP.xs])
                                .width(Length::Fixed(c.width)),
                        );
                        continue;
                    }
                    // A binding that resolves to nothing (#968) shows in
                    // the error colour with a warning glyph, so the row
                    // reads as broken before anyone opens it.
                    let slot = match c.kind {
                        ColumnKind::Symbol => RefSlot::Symbol,
                        _ => RefSlot::Footprint,
                    };
                    let (row_value, color) = if dangling.marks_row(r.row_id, slot) {
                        (format!("⚠ {row_value}"), theme_ext::error_color(tokens))
                    } else {
                        (row_value, theme_ext::text_secondary(tokens))
                    };
                    data_row = data_row.push(
                        container(
                            text(row_value)
                                .size(BROWSER_TEXT_SIZE)
                                .wrapping(iced::widget::text::Wrapping::None)
                                .color(color),
                        )
                        .padding([SP.s, 6.0])
                        .width(Length::Fixed(c.width)),
                    );
                    continue;
                }

                // Read-only mount (the stock library): a plain label,
                // not an input — editing is unavailable up front
                // rather than failing on commit.
                if read_only {
                    data_row = data_row.push(
                        container(
                            text(row_value)
                                .size(BROWSER_TEXT_SIZE)
                                .wrapping(iced::widget::text::Wrapping::None)
                                .color(theme_ext::text_secondary(tokens)),
                        )
                        .padding([SP.s, 6.0])
                        .width(Length::Fixed(c.width)),
                    );
                    continue;
                }

                // A measurement is edited as a number, an SI prefix and
                // its unit. A 90-110 px column cannot hold the number
                // field and the prefix picker beside the cell's text, so
                // the cell opens the Edit Component Details modal, which
                // has both, instead of taking raw text.
                if let Some(open) = c.kind.measurement_press(r, &lib_pb, table) {
                    let text_c = theme_ext::text_primary(tokens);
                    let hover = iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06);
                    let cell = iced::widget::button(
                        text(row_value)
                            .size(BROWSER_TEXT_SIZE)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .color(text_c),
                    )
                    .padding([2, 6])
                    .width(Length::Fill)
                    .on_press(open)
                    .style(move |_: &Theme, status| {
                        iced::widget::button::Style {
                            background: matches!(status, iced::widget::button::Status::Hovered)
                                .then_some(iced::Background::Color(hover)),
                            text_color: text_c,
                            ..iced::widget::button::Style::default()
                        }
                    });
                    data_row = data_row.push(
                        container(cell)
                            .padding([2, 2])
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
                    .padding([SP.xs, 6.0])
                    .size(BROWSER_TEXT_SIZE);
                data_row = data_row.push(
                    container(input)
                        .padding([SP.xs, SP.xs])
                        .width(Length::Fixed(c.width)),
                );
            }

            let library_for_msg = lib_pb.clone();
            let table_for_msg = table.to_string();
            let row_container = container(data_row)
                .padding([0, 0])
                .width(Length::Fill)
                .height(Length::Fixed(BROWSER_ROW_HEIGHT))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: bg_color,
                    border: Border {
                        width: 0.0,
                        radius: 0.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..Default::default()
                });

            // Click selects the row; the Properties panel surfaces its
            // detail and the action row's "Edit Details" button opens
            // the full Edit Component Details modal for it. The
            // text_inputs above capture clicks for focus, so this only
            // fires on the gaps between cells.
            let row_widget = mouse_area(row_container).on_press(LibraryMessage::BrowserSelectRow {
                library_path: library_for_msg,
                table: table_for_msg,
                row_id,
            });

            col = col.push(keyed(r.row_id, row_widget));
        }
        if window.end < rows.len() {
            col = col.push(Space::new().height(Length::Fixed(
                (rows.len() - window.end) as f32 * BROWSER_ROW_HEIGHT,
            )));
        }
    }

    let library_for_scroll = lib_pb.clone();
    let scroller = scrollable(col)
        .id(grid_scroll_id(&lib_pb))
        .on_scroll(move |viewport| LibraryMessage::BrowserGridScrolled {
            library_path: library_for_scroll.clone(),
            offset_y: viewport.absolute_offset().y,
        })
        .width(Length::Fill)
        .height(Length::Fill);
    // The scrollable keeps its own offset in the widget tree. Keyed by
    // library, so switching tabs builds a fresh one instead of showing the
    // previous library's offset against this library's window.
    let library_for_resize = lib_pb.clone();
    // The sensor sits inside the key: its record of what it published
    // belongs to this library and must not survive a switch to another.
    let scroller = keyed(
        library_key(&lib_pb),
        sense_height(
            scroller,
            browser.grid_scroll.viewport_h,
            move |viewport_h| LibraryMessage::BrowserGridResized {
                library_path: library_for_resize.clone(),
                viewport_h,
            },
        ),
    );
    container(scroller)
        .padding([0, 0])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}

/// Warning-coloured cell button for an unbound symbol / footprint that
/// opens the existing primitive picker against this row.
fn unbound_pick_button<'a>(
    label: &'static str,
    kind: signex_library::PrimitiveKind,
    address: crate::library::state::EditorAddress,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let warning = theme_ext::warning_color(tokens);
    iced::widget::button(
        text(label)
            .size(BROWSER_TEXT_SIZE)
            .wrapping(iced::widget::text::Wrapping::None)
            .color(warning),
    )
    .padding([SP.xs, SP.s])
    .on_press(LibraryMessage::OpenPrimitivePicker {
        kind,
        target: crate::library::state::PrimitivePickerTarget::BrowserRow(address),
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: None,
        text_color: warning,
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: warning,
        },
        ..iced::widget::button::Style::default()
    })
    .into()
}
