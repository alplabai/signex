//! Library Browser — detail preview pane (DEAD CODE, F15 final pass).
//!
//! `view_preview_pane` / `preview_panel` / `preview_panel_with_pick` /
//! `symbol_summary` / `footprint_summary` / `short_row_id` are kept as
//! dead code so the prune stays reviewable in one commit. The
//! Properties panel reads `PanelContext.library_row_detail` and renders
//! the equivalent. Pruning this block in the next cleanup pass.
//! Moved verbatim from the former single-file `browser` module.

use super::*;
use iced::widget::column;

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
