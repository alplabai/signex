//! Drawing (Line / Rectangle / Circle / Arc / Polygon) properties
//! surface plus its per-field row builders (numeric row, stroke-colour
//! swatch row, fill-type row). Moved verbatim from the former
//! single-file `element_properties` module.

use super::super::*;
use super::drawing_preview::DrawingPreview;

pub(in crate::panels) fn view_drawing_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    _primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    use signex_types::schematic::FillType;
    let get = |key: &str| -> String {
        ctx.selection_info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let parse_pair = |s: &str| -> (f64, f64) {
        let parts: Vec<&str> = s.split(',').collect();
        let x = parts
            .first()
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let y = parts
            .get(1)
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        (x, y)
    };
    let parse_f64 = |s: &str| -> f64 { s.trim().parse::<f64>().ok().unwrap_or(0.0) };
    let parse_fill = |s: &str| -> FillType {
        match s {
            "Outline" => FillType::Outline,
            "Background" => FillType::Background,
            _ => FillType::None,
        }
    };

    let elem_type = get("Type");
    // Line + Arc store their stroke in the `Width` key; Rect / Circle
    // / Polygon put stroke under `Border` and use `Width` for the
    // X-dimension (Rect) or nothing (Circle / Polygon). Picking the
    // wrong one here pre-fills the input with the X-dim or 0.
    let stroke_w = match elem_type.as_str() {
        "Line" | "Arc" => parse_f64(&get("Width")),
        _ => parse_f64(&get("Border")),
    };
    let fill = parse_fill(&get("Fill"));
    let show_fill = matches!(elem_type.as_str(), "Rectangle" | "Circle" | "Polygon");

    let mut col = Column::new().spacing(0).width(Length::Fill);
    // Header: shape icon + type label. Draft SVGs live at
    // assets/icons/shape_*.svg and can be swapped out for final art
    // without touching the panel code.
    let header_row: Element<'a, PanelMsg> =
        if let Some(icon) = shape_icon_handle(&elem_type, ctx.theme_id) {
            row![
                iced::widget::svg(icon).width(16).height(16),
                text(elem_type.clone())
                    .size(11)
                    .color(Color::from_rgb(0.90, 0.90, 0.92)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            text(elem_type.clone())
                .size(11)
                .color(Color::from_rgb(0.90, 0.90, 0.92))
                .into()
        };
    col = col.push(container(header_row).padding([6, 8]).width(Length::Fill));
    col = col.push(thin_sep(border_c));

    // Live preview canvas — shows the selected shape at panel scale
    // with optional radius/angle annotations so edits to the rows
    // below re-render the preview immediately.
    if let Some(drawing) = &ctx.selected_drawing {
        let preview = DrawingPreview {
            drawing: drawing.clone(),
            stroke: Color::from_rgb(0.94, 0.74, 0.28),
            fill: Color::from_rgb(0.94, 0.74, 0.28),
            muted,
            accent: Color::from_rgb(0.24, 0.62, 0.97),
        };
        let canvas_w: Element<'a, PanelMsg> = iced::widget::canvas(preview)
            .width(Length::Fill)
            .height(Length::Fixed(160.0))
            .into();
        col = col.push(
            container(canvas_w)
                .padding([8, 12])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.07, 0.07, 0.08, 0.6))),
                    border: Border {
                        width: 1.0,
                        color: border_c,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }),
        );
    }

    let buf = &ctx.drawing_edit_buf;
    match elem_type.as_str() {
        "Line" => {
            let (sx, sy) = parse_pair(&get("Start"));
            let (ex, ey) = parse_pair(&get("End"));
            col = col.push(collapsible_section(
                "draw_line",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Start X",
                        DrawingFieldId::LineStartX,
                        sx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Start Y",
                        DrawingFieldId::LineStartY,
                        sy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End X",
                        DrawingFieldId::LineEndX,
                        ex,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End Y",
                        DrawingFieldId::LineEndY,
                        ey,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width (mm)",
                        DrawingFieldId::LineWidth,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    c
                },
            ));
        }
        "Rectangle" => {
            let (px, py) = parse_pair(&get("Position"));
            let w_mm = parse_f64(&get("Width"));
            let h_mm = parse_f64(&get("Height"));
            col = col.push(collapsible_section(
                "draw_rect",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Position X",
                        DrawingFieldId::RectStartX,
                        px,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Position Y",
                        DrawingFieldId::RectStartY,
                        py,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width (mm)",
                        DrawingFieldId::RectWidth,
                        w_mm,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Height (mm)",
                        DrawingFieldId::RectHeight,
                        h_mm,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::RectBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        "Circle" => {
            let (cx, cy) = parse_pair(&get("Center"));
            let radius = parse_f64(&get("Radius"));
            col = col.push(collapsible_section(
                "draw_circle",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Center X",
                        DrawingFieldId::CircleCenterX,
                        cx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Center Y",
                        DrawingFieldId::CircleCenterY,
                        cy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Radius",
                        DrawingFieldId::CircleRadius,
                        radius,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::CircleBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        "Arc" => {
            let (cx, cy) = parse_pair(&get("Center"));
            let radius = parse_f64(&get("Radius"));
            let start_angle = parse_f64(&get("Start Angle"));
            let end_angle = parse_f64(&get("End Angle"));
            col = col.push(collapsible_section(
                "draw_arc",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Center X",
                        DrawingFieldId::ArcCenterX,
                        cx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Center Y",
                        DrawingFieldId::ArcCenterY,
                        cy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Radius",
                        DrawingFieldId::ArcRadius,
                        radius,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Start Angle",
                        DrawingFieldId::ArcStartAngle,
                        start_angle,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End Angle",
                        DrawingFieldId::ArcEndAngle,
                        end_angle,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width",
                        DrawingFieldId::ArcWidth,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    c
                },
            ));
        }
        "Polygon" => {
            let vert_count = parse_f64(&get("Vertices")) as i32;
            col = col.push(collapsible_section(
                "draw_poly",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(prop_kv_row(
                        "Vertices",
                        &vert_count.to_string(),
                        muted,
                        Color::from_rgb(0.90, 0.90, 0.92),
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::PolyBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        _ => {
            for (key, value) in &ctx.selection_info {
                if key != "Type" {
                    col = col.push(prop_kv_row(
                        key,
                        value,
                        muted,
                        Color::from_rgb(0.9, 0.9, 0.92),
                    ));
                }
            }
        }
    }
    // Stroke colour swatch row — applies to every drawing kind that
    // matched a known variant above. Reads the current stored colour
    // from the live SchDrawing so the active tile highlights correctly.
    if matches!(
        elem_type.as_str(),
        "Line" | "Rectangle" | "Circle" | "Arc" | "Polygon"
    ) {
        let current_color = ctx.selected_drawing.as_ref().and_then(|d| match d {
            signex_types::schematic::SchDrawing::Line { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Rect { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Circle { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Arc { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Polyline { stroke_color, .. } => *stroke_color,
        });
        col = col.push(drawing_stroke_color_row(current_color, muted));
    }
    col.into()
}

/// Buffer-backed numeric row — survives empty / partial input so the
/// user can erase and retype the whole value. Emits DrawingFieldTyping
/// on every keystroke; the handler commits to the engine when the
/// string parses as f64.
fn drawing_num_row<'a>(
    label: &'a str,
    field: DrawingFieldId,
    stored_value: f64,
    buf: &std::collections::HashMap<DrawingFieldId, String>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::{row, text, text_input};
    let display = buf
        .get(&field)
        .cloned()
        .unwrap_or_else(|| format!("{stored_value:.3}"));
    row![
        text(label)
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text_input("", &display)
            .size(11)
            .on_input(move |s| PanelMsg::DrawingFieldTyping(field, s))
            .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Altium-style stroke colour swatch row. A small preset palette
/// (Theme/Red/Green/Blue/Yellow/Orange/White/Black) lets the user
/// recolour a placed shape without committing to a full colour
/// picker. Each tile dispatches UpdateDrawingEdit::StrokeColor.
fn drawing_stroke_color_row<'a>(
    current: Option<signex_types::schematic::StrokeColor>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use iced::widget::{button, row, text};
    use signex_types::schematic::StrokeColor;
    let rgb = |r: u8, g: u8, b: u8| -> StrokeColor { StrokeColor { r, g, b, a: 255 } };
    let tile = |label: &'static str,
                stored: Option<StrokeColor>,
                active: bool,
                fill_color: Color|
     -> Element<'a, PanelMsg> {
        button(text(label).size(9))
            .padding([3, 6])
            .on_press(PanelMsg::UpdateDrawingEdit(E::StrokeColor(stored)))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(fill_color)),
                border: Border {
                    width: if active { 2.0 } else { 1.0 },
                    radius: 3.0.into(),
                    color: if active {
                        Color::from_rgb(1.0, 1.0, 1.0)
                    } else {
                        Color::from_rgb(0.28, 0.28, 0.32)
                    },
                },
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    let is_active = |c: Option<StrokeColor>| -> bool {
        match (current, c) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    };
    let theme_active = current.is_none();
    row![
        text("Color")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile(
                "Auto",
                None,
                theme_active,
                Color::from_rgba(0.28, 0.28, 0.32, 0.6),
            ),
            tile(
                "",
                Some(rgb(0xE5, 0x3E, 0x3E)),
                is_active(Some(rgb(0xE5, 0x3E, 0x3E))),
                Color::from_rgb(0.90, 0.24, 0.24),
            ),
            tile(
                "",
                Some(rgb(0x3E, 0xA5, 0x44)),
                is_active(Some(rgb(0x3E, 0xA5, 0x44))),
                Color::from_rgb(0.24, 0.65, 0.27),
            ),
            tile(
                "",
                Some(rgb(0x3C, 0x85, 0xD6)),
                is_active(Some(rgb(0x3C, 0x85, 0xD6))),
                Color::from_rgb(0.24, 0.52, 0.84),
            ),
            tile(
                "",
                Some(rgb(0xE6, 0xB7, 0x1E)),
                is_active(Some(rgb(0xE6, 0xB7, 0x1E))),
                Color::from_rgb(0.90, 0.72, 0.12),
            ),
            tile(
                "",
                Some(rgb(0xE0, 0xE0, 0xE0)),
                is_active(Some(rgb(0xE0, 0xE0, 0xE0))),
                Color::from_rgb(0.88, 0.88, 0.88),
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn drawing_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use signex_types::schematic::FillType;
    let tile = |label: &'static str, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        iced::widget::button(text(label).size(10))
            .padding([3, 8])
            .on_press(PanelMsg::UpdateDrawingEdit(E::Fill(ft)))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(if active {
                    Color::from_rgb(0.20, 0.36, 0.58)
                } else {
                    Color::from_rgba(0.25, 0.25, 0.28, 0.4)
                })),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: Color::from_rgb(0.28, 0.28, 0.32),
                },
                text_color: if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    muted
                },
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    row![
        text("Fill")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile("None", FillType::None, current == FillType::None),
            tile("Outline", FillType::Outline, current == FillType::Outline),
            tile(
                "Background",
                FillType::Background,
                current == FillType::Background,
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}
