//! Symbol-level (nothing-selected) Properties rows + local-colour swatches.

use iced::widget::{Column, Space, column, container, row, text};
use iced::{Color, Element, Length};

use super::super::{PanelMsg, SymbolEditorPanelContext};

/// Render one Local Colors row — three click-to-cycle swatches
/// (Fills / Lines / Pins). Each swatch shows the current override
/// colour or a striped "inherit" pattern when `None`. Clicking
/// cycles through a small preset palette + back to None.
fn local_colors_row<'a>(
    label: &'a str,
    fill: Option<[u8; 4]>,
    line: Option<[u8; 4]>,
    pin: Option<[u8; 4]>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let swatch = |slot_label: &'a str, c: Option<[u8; 4]>, msg: PanelMsg| {
        let bg = match c {
            Some([r, g, b, a]) => iced::Color::from_rgba8(r, g, b, (a as f32) / 255.0),
            None => iced::Color::from_rgba(0.5, 0.5, 0.5, 0.25),
        };
        let border = if c.is_some() {
            iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.30)
        };
        column![
            text(slot_label).size(9).color(muted),
            iced::widget::button(iced::widget::Space::new())
                .padding(0)
                .width(Length::Fixed(28.0))
                .height(Length::Fixed(16.0))
                .on_press(msg)
                .style(
                    move |_: &iced::Theme, _status: iced::widget::button::Status| {
                        iced::widget::button::Style {
                            background: Some(iced::Background::Color(bg)),
                            border: iced::Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border,
                            },
                            ..iced::widget::button::Style::default()
                        }
                    }
                ),
        ]
        .spacing(2)
        .align_x(iced::Alignment::Center)
    };

    container(
        row![
            text(label.to_string())
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            row![
                swatch("Fills", fill, PanelMsg::SymEditorCycleLocalFillColor),
                Space::new().width(8),
                swatch("Lines", line, PanelMsg::SymEditorCycleLocalLineColor),
                Space::new().width(8),
                swatch("Pins", pin, PanelMsg::SymEditorCycleLocalPinColor),
            ]
            .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into()
}

/// Symbol-level default Properties (nothing selected): identity,
/// graphical toggles, and local-colour overrides.
pub(super) fn view_symbol_selection<'a>(
    mut col: Column<'a, PanelMsg>,
    sym: &'a SymbolEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
            // The .snxsym editor is a SYMBOL editor — symbol-level
            // visual / geometric properties only. Component metadata
            // (Designator / Comment / Description / Type / Parameters)
            // lives on the host ComponentRow in the component library
            // and is edited from the Library Browser / Component
            // Editor, not here.

            // Helper — labelled text_input row.
            let text_field = |label: &'a str,
                              value: &'a str,
                              placeholder: &'a str,
                              on_input: fn(String) -> PanelMsg|
             -> Element<'a, PanelMsg> {
                container(
                    row![
                        text(label)
                            .size(10)
                            .color(muted)
                            .width(Length::FillPortion(2)),
                        iced::widget::text_input(placeholder, value)
                            .padding([2, 4])
                            .size(11)
                            .on_input(on_input)
                            .width(Length::FillPortion(3)),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([3, 8])
                .width(Length::Fill)
                .into()
            };

            // ── ▾ Symbol ──
            col = col.push(super::super::thin_sep(border_c));
            col = col.push(container(text("Symbol").size(11).color(primary)).padding([6, 8]));
            col = col.push(text_field(
                "Design Item ID",
                sym.symbol_name.as_str(),
                "Symbol name",
                PanelMsg::SymEditorSetSymbolName,
            ));
            col = col.push(super::prop_row_static(
                "UUID",
                sym.symbol_uuid.to_string(),
                muted,
                primary,
            ));
            col = col.push(super::prop_row_static(
                "Pins",
                sym.pins.len().to_string(),
                muted,
                primary,
            ));
            col = col.push(super::prop_row_static(
                "Graphics",
                sym.graphics.len().to_string(),
                muted,
                primary,
            ));

            // Part of Parts (Altium "Part B / of Parts: 2") — surfaces
            // the multi-part picker the user already drives via the
            // toolbar arrows or the SCH Library tree-expander.
            let part_label = if sym.active_max_part > 1 {
                format!(
                    "Part {} / of Parts {}",
                    sym.active_part, sym.active_max_part
                )
            } else {
                "Single-part".to_string()
            };
            col = col.push(super::prop_row_static("Part", part_label, muted, primary));

            // ── ▾ Graphical ──
            col = col.push(super::super::thin_sep(border_c));
            col = col.push(container(text("Graphical").size(11).color(primary)).padding([6, 8]));
            let mirrored_row: Element<'a, PanelMsg> = container(
                row![
                    text("Mirrored")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::checkbox(sym.symbol_mirrored)
                        .size(14)
                        .on_toggle(|_| PanelMsg::SymEditorToggleSymbolMirrored),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(mirrored_row);

            // ── Local Colors (Fills / Lines / Pins) ──
            col = col.push(local_colors_row(
                "Local Colors",
                sym.symbol_local_fill_color,
                sym.symbol_local_line_color,
                sym.symbol_local_pin_color,
                muted,
            ));
    col
}
