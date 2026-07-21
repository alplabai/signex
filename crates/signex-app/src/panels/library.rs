//! Library panels -- SCH Library, Footprint Library and row detail.

use super::*;
use iced::widget::column;

/// Context passed to panels — owned data to avoid lifetime issues.
#[derive(Debug, Clone)]
pub struct LibrarySymbolEntry {
    pub lib_id: String,
    pub symbol_name: String,
    pub library_name: String,
    pub pin_count: usize,
}

/// Detail for the row currently selected in the active Library
/// Browser tab. Surfaces in the right-edge Properties panel so
/// primitive binding (Pick Symbol / Pick Footprint) lives next to
/// every other "what am I selected?" affordance the user looks for
/// there. F15 (2026-05-03 library polish): "right pane can be opened
/// on properties instead."
///
/// Populated by `refresh_panel_ctx` when the active tab is
/// `TabKind::LibraryBrowser(path)` AND the matching browser state's
/// `selected_row` is `Some(_)`. Cleared otherwise.
#[derive(Debug, Clone)]
pub struct LibraryRowDetail {
    pub library_path: std::path::PathBuf,
    pub table: String,
    pub row_id: uuid::Uuid,
    pub internal_pn: String,
    /// Class string stored on the row — derived from the table name
    /// at create time per F20 (Tables-only model). Surfaced as
    /// read-only metadata only; users can't edit it directly.
    pub class: String,
    /// Pretty `LifecycleState` token ("Draft", "Released", …).
    pub lifecycle_label: String,
    /// "Symbol bound" / "Symbol unresolved (UUID not mounted)" /
    /// "Symbol unbound". Same shape as the legacy preview pane's
    /// `symbol_summary` first line.
    pub symbol_summary: String,
    /// Same shape for footprint — "Footprint bound", "unbound", …
    pub footprint_summary: String,
}

// ─── SCH Library Panel (Altium parity) ───────────────────────────────
//
// When a `.snxsym` standalone editor tab is active, this panel lists
// the symbols in the open `SymbolFile` container. Click switches the
// active symbol; "Add Symbol" appends a fresh empty Symbol to the
// container and makes it active. Read-only on Symbol metadata for now —
// rename / designator-prefix edits go through the right-dock Properties
// panel.
//
// When no Symbol editor is open the panel renders a hint pointing the
// user at the project tree's `Add New ▸ Symbol` flow.

pub fn view_sch_library<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(text("SCH Library").size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    let Some(sym) = ctx.symbol_editor.as_ref() else {
        col = col.push(
            container(
                text(
                    "Open a `.snxsym` to see its symbols here. Right-click a library node \
                     in the project tree and pick `Add New ▸ Symbol` to create one.",
                )
                .size(10)
                .color(muted),
            )
            .padding([6, 8])
            .width(Length::Fill),
        );
        return scrollable(col).width(Length::Fill).into();
    };

    // ── Breadcrumb header — F28 (2026-05-03) ──
    // Shows `<library_name>  >  <symbol_file>  (N symbols)` so the
    // user always knows which `.snxlib` the active `.snxsym` belongs
    // to. Walk `sym.path` ancestors looking for the first directory
    // ending in `.snxlib`; fall back to just the filename when the
    // file lives outside any library.
    let symbol_file = sym
        .path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<untitled>".to_string());
    let library_stem: Option<String> = sym
        .path
        .ancestors()
        .find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("snxlib"))
                .unwrap_or(false)
        })
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    let breadcrumb = match library_stem {
        Some(lib) => format!(
            "{}  ›  {}  ({} symbols)",
            lib,
            symbol_file,
            sym.symbols_in_file.len(),
        ),
        None => format!("{}  ({} symbols)", symbol_file, sym.symbols_in_file.len()),
    };
    col = col.push(container(text(breadcrumb).size(10).color(muted)).padding([4, 8]));
    col = col.push(thin_sep(border_c));

    // ── Column header — Altium SCH Library parity. Two columns:
    //     Design Item ID | Description.
    col = col.push(
        container(
            row![
                text("Design Item ID")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Description")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(4)),
            ]
            .spacing(6),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // ── Symbols tree ──
    // Each symbol is a row showing Design Item ID + Description.
    // Multi-part symbols (or symbols with Part Zero pins) expand
    // under the active symbol with one row per part — click to
    // switch active_part.
    for entry in &sym.symbols_in_file {
        let is_active = entry.idx == sym.active_idx;
        let label_color = if is_active { primary } else { muted };
        let bg_active = crate::styles::ti(ctx.tokens.selection);
        let row_msg = PanelMsg::SchLibrarySelectSymbol(entry.idx);
        let row_btn = iced::widget::button(
            row![
                text(entry.name.clone())
                    .size(11)
                    .color(label_color)
                    .width(Length::FillPortion(3)),
                text(entry.description.clone())
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(4)),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .on_press(row_msg)
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: if is_active {
                        Some(iced::Background::Color(bg_active))
                    } else if matches!(status, iced::widget::button::Status::Hovered) {
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04,
                        )))
                    } else {
                        None
                    },
                    border: iced::Border::default(),
                    text_color: label_color,
                    ..iced::widget::button::Style::default()
                }
            },
        );
        col = col.push(row_btn);

        // Part tree-expander under the active multi-part symbol.
        if is_active && (sym.active_max_part > 1 || sym.active_has_part_zero) {
            if sym.active_has_part_zero {
                col = col.push(part_tree_row(
                    "Part 0 (shared)",
                    0,
                    sym.active_part == 0,
                    primary,
                    muted,
                    bg_active,
                ));
            }
            for part in 1..=sym.active_max_part {
                col = col.push(part_tree_row(
                    &format!("Part {part}"),
                    part,
                    sym.active_part == part,
                    primary,
                    muted,
                    bg_active,
                ));
            }
        }
    }

    col = col.push(thin_sep(border_c));

    // Pins / Graphics sub-lists used to live here; per Altium SCH
    // Library panel parity they belong on dedicated panels (Pins
    // already shows in the Properties panel's Pin selection branch;
    // a dedicated SCHLIB Filter / SCHLIB List pair lives on the
    // bottom panel-tabs strip when that ships).
    col = col.push(thin_sep(border_c));

    // ── Action row: Place / Add / Delete / Edit (Altium parity) ──
    let action_btn_style = move |is_primary: bool, enabled: bool| {
        let text_color = if enabled { primary } else { muted };
        move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg_alpha = if !enabled {
                0.02
            } else if is_primary {
                if matches!(status, iced::widget::button::Status::Hovered) {
                    0.10
                } else {
                    0.04
                }
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                0.08
            } else {
                0.03
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, bg_alpha,
                ))),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                text_color,
                ..iced::widget::button::Style::default()
            }
        }
    };

    // Place — fires on the active symbol (would dispatch to schematic
    // place flow when wired). Stub for now: greyed-out.
    let place_btn = iced::widget::button(text("Place").size(11).color(muted))
        .padding([4, 10])
        .style(action_btn_style(true, false));

    let add_btn = iced::widget::button(text("Add").size(11).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::SchLibraryAddSymbol)
        .style(action_btn_style(false, true));

    let can_delete = sym.symbols_in_file.len() > 1;
    let delete_color = if can_delete { primary } else { muted };
    let mut delete_btn = iced::widget::button(text("Delete").size(11).color(delete_color))
        .padding([4, 10])
        .style(action_btn_style(false, can_delete));
    if can_delete {
        delete_btn = delete_btn.on_press(PanelMsg::SchLibraryDeleteSymbol(sym.active_idx));
    }

    // Edit — opens the active symbol in the standalone editor (for
    // when the SCH Library panel is the only visible surface).
    // Stub for now since the symbol is already open in its tab.
    let edit_btn = iced::widget::button(text("Edit").size(11).color(muted))
        .padding([4, 10])
        .style(action_btn_style(false, false));

    col = col.push(
        container(
            row![
                place_btn,
                Space::new().width(4),
                add_btn,
                Space::new().width(4),
                delete_btn,
                Space::new().width(4),
                edit_btn,
                Space::new().width(Length::Fill),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 8]),
    );

    scrollable(col).width(Length::Fill).into()
}

/// v0.18.8 — Footprint Library panel. Mirror of Altium's PCB Library
/// panel: rows are the footprints *inside* the active `.snxfpt`
/// envelope (one per `file.footprints[i]`), with a Place / Add /
/// Delete / Edit button row at the bottom. Single-click highlights;
/// double-click (or Edit) promotes the selection to `active_idx`.
///
/// Cross-file navigation (sibling `.snxfpt` files inside the same
/// `.snxlib`) is reachable through the project tree — keeping the
/// panel single-purpose so the button row's targets are unambiguous.
pub fn view_footprint_library<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);
    let bg_active = crate::styles::ti(ctx.tokens.selection);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(text("Footprint Library").size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    let Some(fp) = ctx.footprint_editor.as_ref() else {
        col = col.push(
            container(
                text(
                    "Open a `.snxfpt` to see its footprints here. Right-click a \
                     `.snxlib` (or a project) in the project tree and pick \
                     `Add New ▸ Footprint Library` to create one.",
                )
                .size(10)
                .color(muted),
            )
            .padding([6, 8])
            .width(Length::Fill),
        );
        return scrollable(col).width(Length::Fill).into();
    };

    // Breadcrumb — file name + footprint count.
    let file_name = fp
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("<unknown>")
        .to_string();
    let breadcrumb = format!(
        "{}  ({} footprint{})",
        file_name,
        fp.internal_footprints.len(),
        if fp.internal_footprints.len() == 1 {
            ""
        } else {
            "s"
        },
    );
    col = col.push(container(text(breadcrumb).size(10).color(muted)).padding([4, 8]));
    col = col.push(thin_sep(border_c));

    // Two-column header: Name + Pads (right-aligned count).
    col = col.push(
        container(
            row![
                text("Name").size(10).color(muted).width(Length::Fill),
                text("Pads").size(10).color(muted),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // Internal-footprint rows.
    for (idx, footprint_row) in fp.internal_footprints.iter().enumerate() {
        let is_selected = fp.internal_selected_idx == Some(idx);
        let is_active = footprint_row.is_active;
        // Active row paints with the selection tint; selected-only
        // (not yet active) paints with a slightly lighter tint so the
        // user can tell selection apart from "currently editing".
        let bg = if is_active {
            iced::Background::Color(bg_active)
        } else if is_selected {
            iced::Background::Color(iced::Color {
                a: 0.4,
                ..bg_active
            })
        } else {
            iced::Background::Color(iced::Color::TRANSPARENT)
        };
        let label_color = if is_active || is_selected {
            primary
        } else {
            muted
        };
        let row_btn = iced::widget::button(
            row![
                text(footprint_row.name.clone())
                    .size(10)
                    .color(label_color)
                    .width(Length::Fill),
                text(footprint_row.pad_count.to_string())
                    .size(10)
                    .color(label_color),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 8])
        .on_press(PanelMsg::FpLibrarySelectInternal(idx))
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(bg),
            border: iced::Border {
                width: 0.0,
                radius: 0.0.into(),
                color: iced::Color::TRANSPARENT,
            },
            ..iced::widget::button::Style::default()
        })
        .width(Length::Fill);
        col = col.push(row_btn);
    }

    // Place / Add / Delete / Edit button row pinned at the bottom of
    // the panel (Altium PCB Library parity). Place / Delete / Edit
    // require a selection; greyed when `internal_selected_idx` is
    // None. Add is always live.
    let selected = fp.internal_selected_idx;
    let footer = view_footprint_library_button_row(ctx, selected);

    iced::widget::column![
        scrollable(col).width(Length::Fill).height(Length::Fill),
        thin_sep(border_c),
        footer,
    ]
    .into()
}

/// Bottom button row for the Footprint Library panel — Altium's
/// `Place / Add / Delete / Edit` quartet.
fn view_footprint_library_button_row<'a>(
    ctx: &'a PanelContext,
    selected: Option<usize>,
) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);

    let mk_btn = |label: &'static str, on_press: Option<PanelMsg>| -> Element<'a, PanelMsg> {
        let enabled = on_press.is_some();
        let label_color = if enabled { primary } else { muted };
        let mut btn = iced::widget::button(
            text(label)
                .size(11)
                .color(label_color)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([4, 12])
        .width(Length::Fixed(64.0))
        .style(move |_: &Theme, status| {
            let bg = match status {
                iced::widget::button::Status::Hovered if enabled => {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                }
                _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.02),
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..iced::widget::button::Style::default()
            }
        });
        if let Some(msg) = on_press {
            btn = btn.on_press(msg);
        }
        btn.into()
    };

    let place = mk_btn(
        "Place",
        // PCB integration not wired yet — keep the button visible
        // but disabled to advertise the intended affordance.
        selected
            .map(PanelMsg::FpLibraryPlaceInternal)
            .filter(|_| false),
    );
    let add = mk_btn("Add", Some(PanelMsg::FpLibraryAddInternal));
    let delete = mk_btn("Delete", selected.map(PanelMsg::FpLibraryDeleteInternal));
    let edit = mk_btn("Edit", selected.map(PanelMsg::FpLibraryEditInternal));

    container(
        row![place, add, delete, edit]
            .spacing(4)
            .align_y(iced::Alignment::Center),
    )
    .padding([6, 8])
    .width(Length::Fill)
    .into()
}

/// F15 — Library Browser row detail in the Properties panel. Shows
/// the row's identifier line + Symbol / Footprint binding status with
/// Pick buttons. Mirrors what the Library Browser's inline preview
/// pane used to render; surfacing here means the user gets the row's
/// detail in the canonical "selected thing" panel, freeing horizontal
/// space inside the browser tab for the grid.
pub fn view_library_row_properties<'a>(
    d: &'a LibraryRowDetail,
    muted: iced::Color,
    primary: iced::Color,
    border_c: iced::Color,
    tokens: &'a ThemeTokens,
) -> Element<'a, PanelMsg> {
    let _ = tokens;
    // Truncate the row UUID to its first 8 hex chars for a
    // human-scannable identity line.
    let row_id_short = {
        let s = d.row_id.simple().to_string();
        if s.len() >= 8 { s[..8].to_string() } else { s }
    };
    let pn_text = if d.internal_pn.is_empty() {
        "(unnamed row)".to_string()
    } else {
        d.internal_pn.clone()
    };

    let pick_symbol_btn = button(text("Pick Symbol…").size(11).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::LibraryRowPickSymbol)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: primary,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
            },
            ..iced::widget::button::Style::default()
        });

    let pick_footprint_btn = button(text("Pick Footprint…").size(11).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::LibraryRowPickFootprint)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: primary,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
            },
            ..iced::widget::button::Style::default()
        });

    let body = column![
        text("Library Row").size(11).color(muted),
        Space::new().height(4),
        text(pn_text).size(13).color(primary),
        Space::new().height(2),
        text(format!(
            "table: {}  ·  class: {}  ·  {}  ·  row {}",
            d.table, d.class, d.lifecycle_label, row_id_short,
        ))
        .size(10)
        .color(muted),
        Space::new().height(12),
        text("Symbol").size(11).color(muted),
        Space::new().height(4),
        text(d.symbol_summary.clone()).size(11).color(primary),
        Space::new().height(6),
        pick_symbol_btn,
        Space::new().height(14),
        text("Footprint").size(11).color(muted),
        Space::new().height(4),
        text(d.footprint_summary.clone()).size(11).color(primary),
        Space::new().height(6),
        pick_footprint_btn,
    ]
    .spacing(0)
    .padding(10);

    container(body).width(Length::Fill).into()
}
