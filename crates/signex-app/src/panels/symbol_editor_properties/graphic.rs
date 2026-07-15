//! Graphic-selection Properties rows for the symbol editor.

use iced::widget::{Column, container, row, text};
use iced::{Color, Element, Length};

use super::super::{GraphicFieldId, GraphicKindSummary, GraphicSummary, PanelMsg};

/// Per-shape numeric Properties rows for a placed graphic (corners /
/// endpoints / centre + radius / arc angles / text + stroke).
pub(super) fn view_graphic_selection<'a>(
    mut col: Column<'a, PanelMsg>,
    g: &'a GraphicSummary,
    muted: Color,
) -> Column<'a, PanelMsg> {
            let g_idx = g.idx;
            // Per-shape numeric fields.
            let num_field =
                |label: &'static str, field: GraphicFieldId, value: f64| -> Element<'a, PanelMsg> {
                    container(
                        row![
                            text(label.to_string())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2)),
                            iced::widget::text_input("mm", &format!("{:.3}", value))
                                .padding([2, 4])
                                .size(11)
                                .on_input(move |s| {
                                    let parsed = s.trim().parse::<f64>().unwrap_or(value);
                                    PanelMsg::SymEditorSetGraphicField {
                                        idx: g_idx,
                                        field,
                                        value: parsed,
                                    }
                                })
                                .width(Length::FillPortion(3)),
                        ]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .into()
                };

            // Fill swatch (closed shapes only) — one click-to-cycle
            // control that both toggles the fill on/off and picks the
            // colour: None (unfilled) → preset palette → None. Mirrors
            // the symbol-level Local Colors swatches.
            let fill_row = |current: Option<[u8; 4]>| -> Element<'a, PanelMsg> {
                let bg = match current {
                    Some([r, gg, b, a]) => Color::from_rgba8(r, gg, b, (a as f32) / 255.0),
                    None => Color::from_rgba(0.5, 0.5, 0.5, 0.25),
                };
                let border_c = if current.is_some() {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.35)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.30)
                };
                let state_label = if current.is_some() { "Solid" } else { "None" };
                container(
                    row![
                        text("Fill")
                            .size(10)
                            .color(muted)
                            .width(Length::FillPortion(2)),
                        row![
                            iced::widget::button(iced::widget::Space::new())
                                .padding(0)
                                .width(Length::Fixed(28.0))
                                .height(Length::Fixed(16.0))
                                .on_press(PanelMsg::SymEditorCycleGraphicFill { idx: g_idx })
                                .style(
                                    move |_: &iced::Theme,
                                          _s: iced::widget::button::Status| {
                                        iced::widget::button::Style {
                                            background: Some(iced::Background::Color(bg)),
                                            border: iced::Border {
                                                width: 1.0,
                                                radius: 2.0.into(),
                                                color: border_c,
                                            },
                                            ..iced::widget::button::Style::default()
                                        }
                                    }
                                ),
                            iced::widget::Space::new().width(8),
                            text(state_label).size(10).color(muted),
                        ]
                        .align_y(iced::Alignment::Center)
                        .width(Length::FillPortion(3)),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([3, 8])
                .width(Length::Fill)
                .into()
            };

            match &g.kind {
                GraphicKindSummary::Rectangle { from, to } => {
                    col = col.push(num_field("From X", GraphicFieldId::FromX, from[0]));
                    col = col.push(num_field("From Y", GraphicFieldId::FromY, from[1]));
                    col = col.push(num_field("To X", GraphicFieldId::ToX, to[0]));
                    col = col.push(num_field("To Y", GraphicFieldId::ToY, to[1]));
                    col = col.push(fill_row(g.fill));
                }
                GraphicKindSummary::Line { from, to } => {
                    col = col.push(num_field("Start X", GraphicFieldId::FromX, from[0]));
                    col = col.push(num_field("Start Y", GraphicFieldId::FromY, from[1]));
                    col = col.push(num_field("End X", GraphicFieldId::ToX, to[0]));
                    col = col.push(num_field("End Y", GraphicFieldId::ToY, to[1]));
                }
                GraphicKindSummary::Circle { center, radius } => {
                    col = col.push(num_field("Center X", GraphicFieldId::CenterX, center[0]));
                    col = col.push(num_field("Center Y", GraphicFieldId::CenterY, center[1]));
                    col = col.push(num_field("Radius", GraphicFieldId::Radius, *radius));
                    col = col.push(fill_row(g.fill));
                }
                GraphicKindSummary::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    col = col.push(num_field("Center X", GraphicFieldId::CenterX, center[0]));
                    col = col.push(num_field("Center Y", GraphicFieldId::CenterY, center[1]));
                    col = col.push(num_field("Radius", GraphicFieldId::Radius, *radius));
                    col = col.push(num_field(
                        "Start \u{00B0}",
                        GraphicFieldId::StartDeg,
                        *start_deg,
                    ));
                    col = col.push(num_field("End \u{00B0}", GraphicFieldId::EndDeg, *end_deg));
                }
                GraphicKindSummary::Text {
                    position,
                    content,
                    size: text_size,
                } => {
                    col = col.push(num_field("X", GraphicFieldId::PositionX, position[0]));
                    col = col.push(num_field("Y", GraphicFieldId::PositionY, position[1]));
                    col = col.push(num_field("Size", GraphicFieldId::TextSize, *text_size));
                    let content_row: Element<'a, PanelMsg> = container(
                        row![
                            text("Content")
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2)),
                            iced::widget::text_input("text", content.as_str())
                                .padding([2, 4])
                                .size(11)
                                .on_input(move |s| PanelMsg::SymEditorSetGraphicText {
                                    idx: g_idx,
                                    value: s,
                                })
                                .width(Length::FillPortion(3)),
                        ]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .into();
                    col = col.push(content_row);
                }
            }
            // Stroke width — common to every variant.
            col = col.push(num_field(
                "Stroke (mm)",
                GraphicFieldId::StrokeWidth,
                g.stroke_width,
            ));
    col
}
