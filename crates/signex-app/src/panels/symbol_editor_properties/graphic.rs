//! Graphic-selection Properties rows for the symbol editor.

use iced::widget::{Column, container, row, text};
use iced::{Color, Element, Length};

use super::super::{
    ColorFieldProps, GraphicFieldId, GraphicKindSummary, GraphicSummary, PanelMsg, color_field,
};

/// Per-shape numeric Properties rows for a placed graphic (corners /
/// endpoints / centre + radius / arc angles / text + stroke).
pub(super) fn view_graphic_selection<'a>(
    mut col: Column<'a, PanelMsg>,
    g: &'a GraphicSummary,
    muted: Color,
    border_c: Color,
    fill_picker: Option<crate::app::GraphicFillPicker>,
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

            match &g.kind {
                GraphicKindSummary::Rectangle { from, to } => {
                    col = col.push(num_field("From X", GraphicFieldId::FromX, from[0]));
                    col = col.push(num_field("From Y", GraphicFieldId::FromY, from[1]));
                    col = col.push(num_field("To X", GraphicFieldId::ToX, to[0]));
                    col = col.push(num_field("To Y", GraphicFieldId::ToY, to[1]));
                    col = col.push(graphic_fill_field(g_idx, g.fill, muted, border_c, fill_picker));
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
                    col = col.push(graphic_fill_field(g_idx, g.fill, muted, border_c, fill_picker));
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

/// Fill colour row for a closed graphic (Rectangle / Circle) — the
/// shared [`color_field`] widget wired to the graphic-fill messages.
/// `picker` carries the transient open-state; the palette / HSV overlay
/// only expands when the picker targets this graphic's index.
fn graphic_fill_field<'a>(
    idx: usize,
    fill: Option<[u8; 4]>,
    muted: Color,
    border_c: Color,
    picker: Option<crate::app::GraphicFillPicker>,
) -> Element<'a, PanelMsg> {
    use std::rc::Rc;

    let this = picker.filter(|p| p.idx == idx);
    let show_palette = this.is_some();
    let show_advanced = this.map(|p| p.advanced).unwrap_or(false);

    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> =
        Rc::new(move |rgba| PanelMsg::SymEditorSetGraphicFill { idx, color: rgba });
    let on_clear = fill
        .is_some()
        .then_some(PanelMsg::SymEditorClearGraphicFill { idx });

    color_field(ColorFieldProps {
        label: "Fill",
        current: fill,
        none_label: "None",
        show_palette,
        show_advanced,
        muted,
        border_c,
        on_toggle: PanelMsg::SymEditorToggleGraphicFillPicker { idx },
        on_advanced: PanelMsg::SymEditorOpenGraphicFillAdvanced { idx },
        on_cancel: PanelMsg::SymEditorCancelGraphicFillPicker,
        on_pick,
        on_clear,
    })
}
