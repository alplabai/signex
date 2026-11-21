//! Properties of a drawing shape (mockup row 48): the line-art illustration at the
//! top, then Location (Position and Size; Center for an arc or a circle;
//! Start and End for a line) → Properties (Border; Radius, Angles, Width)
//! → Appearance (Color, Fill, Placement). The State ▸ Locked section is
//! the shared one `selected` adds. Paired values share a row, and every
//! kind keeps its own fields.

use super::super::*;
use super::illustrations::drawing_shape::{
    DRAW_ANGLES, DRAW_BORDER, DRAW_CENTER, DRAW_ENDS, DRAW_FILL, DRAW_PLACEMENT, DRAW_RADIUS,
    DRAW_SIZE, DRAW_WIDTH, ShapeFacts, shape_illustration,
};
use crate::app::contracts::DrawingFieldEdit as E;
use crate::panels::form_look::{self, FormLook};
use crate::panels::multi_selection::{
    Cell, PairField, control_row, hover_linked, illustration_view, pair_row, segmented_cells,
};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::schematic::{DrawingPlacement, FillType, SchDrawing, StrokeColor};
use signex_widgets::property_illustration::FieldId;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The selection-info rows of the selected drawing, parsed.
struct Rows<'a> {
    info: &'a [(String, String)],
}

impl Rows<'_> {
    fn get(&self, key: &str) -> String {
        self.info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    fn number(&self, key: &str) -> f64 {
        crate::numeric_field::parse_field_f64(&self.get(key)).unwrap_or(0.0)
    }

    fn pair(&self, key: &str) -> (f64, f64) {
        let text = self.get(key);
        let mut parts = text.split(',');
        let mut next = || {
            parts
                .next()
                .and_then(crate::numeric_field::parse_field_f64)
                .unwrap_or(0.0)
        };
        let x = next();
        (x, next())
    }

    fn fill(&self) -> FillType {
        match self.get("Fill").as_str() {
            "Outline" => FillType::Outline,
            "Background" => FillType::Background,
            _ => FillType::None,
        }
    }
}

/// What the illustration of the selected kind needs.
fn shape_facts(
    kind: &str,
    rows: &Rows<'_>,
    drawing: Option<&SchDrawing>,
    placement: DrawingPlacement,
) -> Option<ShapeFacts> {
    Some(match kind {
        "Line" => ShapeFacts::Line {
            start: rows.pair("Start"),
            end: rows.pair("End"),
        },
        "Rectangle" => ShapeFacts::Rect {
            size: (rows.number("Width"), rows.number("Height")),
            fill: rows.fill(),
            placement,
        },
        "Circle" => ShapeFacts::Circle { fill: rows.fill() },
        "Arc" => ShapeFacts::Arc {
            start_deg: rows.number("Start Angle"),
            end_deg: rows.number("End Angle"),
        },
        "Polygon" => ShapeFacts::Polygon {
            points: match drawing {
                Some(SchDrawing::Polyline { points, .. }) => {
                    points.iter().map(|p| (p.x, p.y)).collect()
                }
                _ => Vec::new(),
            },
            fill: rows.fill(),
        },
        _ => return None,
    })
}

/// The text a numeric box shows: the draft while typed, else the value.
fn shown(field: DrawingFieldId, stored: f64, ctx: &PanelContext) -> String {
    ctx.drawing_edit_buf
        .get(&field)
        .cloned()
        .unwrap_or_else(|| format!("{stored:.3}"))
}

/// One half of a paired row: axis mark, box and unit. Each parsable
/// keystroke is applied, as in every drawing field.
fn half<'a>(
    ctx: &PanelContext,
    axis: MessageId,
    field: DrawingFieldId,
    stored: f64,
    unit: MessageId,
) -> PairField<'a> {
    PairField {
        axis: t!(ctx.i18n, axis),
        value: shown(field, stored, ctx),
        unit: t!(ctx.i18n, unit),
        on_input: Box::new(move |s| PanelMsg::DrawingFieldTyping(field, s)),
        on_submit: None,
    }
}

/// A label | box | unit row of one drawing field.
fn num_row<'a>(
    ctx: &PanelContext,
    label: MessageId,
    field: DrawingFieldId,
    stored: f64,
    unit: MessageId,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let input = signex_widgets::text_input(
        signex_i18n::UserText::new(String::new()),
        shown(field, stored, ctx),
    )
    .size(TS.body)
    .padding(SP.s)
    .width(Length::Fill)
    .on_input(move |s| PanelMsg::DrawingFieldTyping(field, s))
    .style(form_look::text_input_style);
    let control = row![
        input,
        text(t!(ctx.i18n, unit)).size(TS.caption).color(muted)
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed();
    control_row(t!(ctx.i18n, label), control, muted)
}

/// A paired row: `label` | first | second, linked to `link`.
fn pair<'a>(
    ctx: &'a PanelContext,
    link: FieldId,
    label: MessageId,
    (ax_a, field_a, a): (MessageId, DrawingFieldId, f64),
    (ax_b, field_b, b): (MessageId, DrawingFieldId, f64),
    unit: MessageId,
    muted: Color,
) -> Element<'a, PanelMsg> {
    hover_linked(
        ctx,
        link,
        pair_row(
            t!(ctx.i18n, label),
            half(ctx, ax_a, field_a, a, unit),
            half(ctx, ax_b, field_b, b, unit),
            muted,
        ),
    )
}

/// One Properties section's rows, top to bottom.
type SectionRows<'a> = Vec<Element<'a, PanelMsg>>;

/// Location and Properties of one kind, as `(location, properties)` rows.
fn kind_rows<'a>(
    ctx: &'a PanelContext,
    kind: &str,
    rows: &Rows<'_>,
    muted: Color,
) -> Option<(SectionRows<'a>, SectionRows<'a>)> {
    use DrawingFieldId as F;
    let (x, y, w, h, mm, deg) = (
        ids::PROPERTIES_AXIS_X,
        ids::PROPERTIES_AXIS_Y,
        ids::PROPERTIES_AXIS_W,
        ids::PROPERTIES_AXIS_H,
        ids::PROPERTIES_UNIT_MM,
        ids::PROPERTIES_UNIT_DEGREES,
    );
    let linked = |link: FieldId, row: Element<'a, PanelMsg>| hover_linked(ctx, link, row);
    Some(match kind {
        "Line" => {
            let ((sx, sy), (ex, ey)) = (rows.pair("Start"), rows.pair("End"));
            let start = (x, F::LineStartX, sx);
            let end = (x, F::LineEndX, ex);
            (
                vec![
                    pair(
                        ctx,
                        DRAW_ENDS,
                        ids::PROPERTIES_DRAWING_START,
                        start,
                        (y, F::LineStartY, sy),
                        mm,
                        muted,
                    ),
                    pair(
                        ctx,
                        DRAW_ENDS,
                        ids::PROPERTIES_DRAWING_END,
                        end,
                        (y, F::LineEndY, ey),
                        mm,
                        muted,
                    ),
                ],
                vec![linked(
                    DRAW_WIDTH,
                    num_row(
                        ctx,
                        ids::PROPERTIES_DRAWING_WIDTH,
                        F::LineWidth,
                        rows.number("Width"),
                        mm,
                        muted,
                    ),
                )],
            )
        }
        "Rectangle" => {
            let (px, py) = rows.pair("Position");
            let (rw, rh) = (rows.number("Width"), rows.number("Height"));
            (
                vec![
                    pair(
                        ctx,
                        DRAW_SIZE,
                        ids::PROPERTIES_DRAWING_POSITION,
                        (x, F::RectStartX, px),
                        (y, F::RectStartY, py),
                        mm,
                        muted,
                    ),
                    pair(
                        ctx,
                        DRAW_SIZE,
                        ids::PROPERTIES_DRAWING_SIZE,
                        (w, F::RectWidth, rw),
                        (h, F::RectHeight, rh),
                        mm,
                        muted,
                    ),
                ],
                vec![linked(
                    DRAW_BORDER,
                    num_row(
                        ctx,
                        ids::PANELS_DRAWING_BORDER,
                        F::RectBorder,
                        rows.number("Border"),
                        mm,
                        muted,
                    ),
                )],
            )
        }
        "Circle" => {
            let (cx, cy) = rows.pair("Center");
            (
                vec![pair(
                    ctx,
                    DRAW_CENTER,
                    ids::PROPERTIES_DRAWING_CENTER,
                    (x, F::CircleCenterX, cx),
                    (y, F::CircleCenterY, cy),
                    mm,
                    muted,
                )],
                vec![
                    linked(
                        DRAW_RADIUS,
                        num_row(
                            ctx,
                            ids::PROPERTIES_DRAWING_RADIUS,
                            F::CircleRadius,
                            rows.number("Radius"),
                            mm,
                            muted,
                        ),
                    ),
                    linked(
                        DRAW_BORDER,
                        num_row(
                            ctx,
                            ids::PANELS_DRAWING_BORDER,
                            F::CircleBorder,
                            rows.number("Border"),
                            mm,
                            muted,
                        ),
                    ),
                ],
            )
        }
        "Arc" => {
            let (cx, cy) = rows.pair("Center");
            let angles = (
                (
                    ids::PROPERTIES_ANGLE_START,
                    F::ArcStartAngle,
                    rows.number("Start Angle"),
                ),
                (
                    ids::PROPERTIES_ANGLE_END,
                    F::ArcEndAngle,
                    rows.number("End Angle"),
                ),
            );
            (
                vec![pair(
                    ctx,
                    DRAW_CENTER,
                    ids::PROPERTIES_DRAWING_CENTER,
                    (x, F::ArcCenterX, cx),
                    (y, F::ArcCenterY, cy),
                    mm,
                    muted,
                )],
                vec![
                    linked(
                        DRAW_RADIUS,
                        num_row(
                            ctx,
                            ids::PROPERTIES_DRAWING_RADIUS,
                            F::ArcRadius,
                            rows.number("Radius"),
                            mm,
                            muted,
                        ),
                    ),
                    pair(
                        ctx,
                        DRAW_ANGLES,
                        ids::PROPERTIES_DRAWING_ANGLES,
                        angles.0,
                        angles.1,
                        deg,
                        muted,
                    ),
                    linked(
                        DRAW_WIDTH,
                        num_row(
                            ctx,
                            ids::PROPERTIES_DRAWING_WIDTH,
                            F::ArcWidth,
                            rows.number("Width"),
                            mm,
                            muted,
                        ),
                    ),
                ],
            )
        }
        "Polygon" => {
            let vertices = rows.number("Vertices") as i64;
            let ink = crate::styles::ti(ctx.tokens.text);
            (
                Vec::new(),
                vec![
                    prop_kv_row(
                        t!(ctx.i18n, ids::PROPERTIES_VERTICES),
                        &ctx.i18n.count(vertices),
                        muted,
                        ink,
                    ),
                    linked(
                        DRAW_BORDER,
                        num_row(
                            ctx,
                            ids::PANELS_DRAWING_BORDER,
                            F::PolyBorder,
                            rows.number("Border"),
                            mm,
                            muted,
                        ),
                    ),
                ],
            )
        }
        _ => return None,
    })
}

/// A collapsible section holding `rows`; nothing when there are none.
fn section<'a>(
    ctx: &'a PanelContext,
    key: &str,
    title: MessageId,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    if rows.is_empty() {
        return None;
    }
    Some(
        ctx.collapsible(key, title, &ctx.collapsed_sections, move || {
            Column::with_children(rows).spacing(0).width(Length::Fill)
        })
        .boxed(),
    )
}

pub(in crate::panels) fn view_drawing_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    _primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let rows = Rows {
        info: &ctx.selection_info,
    };
    let kind = rows.get("Type");
    let ink = crate::styles::ti(ctx.tokens.text);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    // Header: shape icon + type label. Draft SVGs live at
    // assets/icons/shape_*.svg and can be swapped out for final art
    // without touching the panel code.
    let header_row: Element<'a, PanelMsg> =
        if let Some(icon) = shape_icon_handle(&kind, ctx.theme_id) {
            row![
                iced::widget::svg(icon).width(TS.label).height(TS.label),
                // raw: the engine's type name has no catalog message yet (#2060).
                text(raw(kind.clone())).size(TS.body).color(ink),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center)
            .boxed()
        } else {
            // raw: the engine's type name has no catalog message yet (#2060).
            text(raw(kind.clone())).size(TS.body).color(ink).boxed()
        };
    col = col.push(
        container(header_row)
            .padding([6.0, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    col = col.push(thin_sep(border_c));

    let placement = ctx.selected_drawing_placement;
    if let Some(facts) = shape_facts(&kind, &rows, ctx.selected_drawing.as_ref(), placement) {
        col = col.push(illustration_view(
            ctx,
            shape_illustration(&facts, &ctx.i18n),
        ));
    }

    let Some((location, properties)) = kind_rows(ctx, &kind, &rows, muted) else {
        for (key, value) in &ctx.selection_info {
            if key != "Type" {
                // raw: the key is worded by signex-engine's selection info (#2060).
                col = col.push(prop_kv_row(raw(key.clone()), value, muted, ink));
            }
        }
        return col.boxed();
    };
    if let Some(s) = section(ctx, "draw_location", ids::PANELS_PLACED_LOCATION, location) {
        col = col.push(s);
    }
    if let Some(s) = section(
        ctx,
        "draw_properties",
        ids::PANELS_PLACED_PROPERTIES,
        properties,
    ) {
        col = col.push(s);
    }
    let has_fill = matches!(kind.as_str(), "Rectangle" | "Circle" | "Polygon");
    let mut appearance = vec![drawing_stroke_color_row(
        current_color(ctx.selected_drawing.as_ref()),
        muted,
        &ctx.i18n,
    )];
    if has_fill {
        appearance.push(hover_linked(
            ctx,
            DRAW_FILL,
            fill_row(rows.fill(), muted, &ctx.i18n),
        ));
    }
    appearance.push(hover_linked(
        ctx,
        DRAW_PLACEMENT,
        placement_row(placement, muted, &ctx.i18n),
    ));
    if let Some(s) = section(
        ctx,
        "draw_appearance",
        ids::PROPERTIES_SECTION_APPEARANCE,
        appearance,
    ) {
        col = col.push(s);
    }
    col.boxed()
}

/// The stored stroke colour of the selected drawing.
fn current_color(drawing: Option<&SchDrawing>) -> Option<StrokeColor> {
    drawing.and_then(|d| match d {
        SchDrawing::Line { stroke_color, .. }
        | SchDrawing::Rect { stroke_color, .. }
        | SchDrawing::Circle { stroke_color, .. }
        | SchDrawing::Arc { stroke_color, .. }
        | SchDrawing::Polyline { stroke_color, .. } => *stroke_color,
        SchDrawing::Bezier(b) => b.stroke_color,
        SchDrawing::Dimension(d) => d.stroke_color,
        SchDrawing::Table(_) => None,
    })
}

/// The stroke colour swatches: Auto (the theme's drawing colour) and five
/// preset document colours. Each tile is one undoable edit.
fn drawing_stroke_color_row<'a>(
    current: Option<StrokeColor>,
    muted: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    use signex_widgets::{button, raw, text};
    let rgb = |r: u8, g: u8, b: u8| -> StrokeColor { StrokeColor { r, g, b, a: 255 } };
    let swatch = |c: StrokeColor| Color::from_rgb8(c.r, c.g, c.b);
    // Only the Auto tile has a label; a colour tile is its swatch.
    let tile = |label: Option<MessageId>, stored: Option<StrokeColor>| -> Element<'a, PanelMsg> {
        let active = current == stored;
        let face = match label {
            Some(id) => text(t!(i18n, id)),
            // raw: an empty face for a colour swatch, no text.
            None => text(raw("")),
        };
        button(face.size(TS.body))
            .padding([3, 6])
            .on_press(PanelMsg::UpdateDrawingEdit(E::StrokeColor(stored)))
            .style(move |theme: &Theme, _| {
                let look = FormLook::of(theme);
                iced::widget::button::Style {
                    background: Some(Background::Color(stored.map_or(look.sunken, swatch))),
                    border: Border {
                        width: if active { 2.0 } else { 1.0 },
                        radius: 3.0.into(),
                        color: if active {
                            look.accent
                        } else {
                            look.line_strong
                        },
                        ..Border::default()
                    },
                    text_color: look.ink,
                    ..iced::widget::button::Style::default()
                }
            })
            .boxed()
    };
    let presets = [
        rgb(0xE5, 0x3E, 0x3E),
        rgb(0x3E, 0xA5, 0x44),
        rgb(0x3C, 0x85, 0xD6),
        rgb(0xE6, 0xB7, 0x1E),
        rgb(0xE0, 0xE0, 0xE0),
    ];
    let mut tiles = vec![tile(Some(ids::PANELS_DRAWING_COLOR_AUTO), None)];
    tiles.extend(presets.into_iter().map(|c| tile(None, Some(c))));
    control_row(
        t!(i18n, ids::PANELS_DRAWING_COLOR),
        iced::widget::Row::with_children(tiles)
            .spacing(SP.s)
            .width(Length::Fill)
            .boxed(),
        muted,
    )
}

/// Placement: Behind circuit / Above circuit. Drawings default to behind,
/// so a highlight or callout laid over symbols and wires is the one that
/// needs choosing (#1583).
fn placement_row<'a>(
    current: DrawingPlacement,
    muted: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let cells = vec![
        Cell::new(
            DrawingPlacement::Behind,
            t!(i18n, ids::PANELS_DRAWING_PLACEMENT_BEHIND),
        ),
        Cell::new(
            DrawingPlacement::Above,
            t!(i18n, ids::PANELS_DRAWING_PLACEMENT_ABOVE),
        ),
    ];
    control_row(
        t!(i18n, ids::PANELS_DRAWING_PLACEMENT),
        segmented_cells(cells, Some(current), 2, |p| {
            PanelMsg::UpdateDrawingEdit(E::Placement(p))
        }),
        muted,
    )
}

/// Fill: None / Outline / Solid, each with a tooltip; Solid fills with the
/// sheet background.
fn fill_row<'a>(current: FillType, muted: Color, i18n: &I18n) -> Element<'a, PanelMsg> {
    let cells = vec![
        Cell::new(FillType::None, t!(i18n, ids::PANELS_SHAPE_FILL_NONE))
            .tip(t!(i18n, ids::PROPERTIES_FILL_NONE_TIP)),
        Cell::new(FillType::Outline, t!(i18n, ids::PANELS_SHAPE_FILL_OUTLINE))
            .tip(t!(i18n, ids::PROPERTIES_FILL_OUTLINE_TIP)),
        Cell::new(FillType::Background, t!(i18n, ids::PROPERTIES_FILL_SOLID))
            .tip(t!(i18n, ids::PROPERTIES_FILL_SOLID_TIP)),
    ];
    control_row(
        t!(i18n, ids::PANELS_SHAPE_FILL),
        segmented_cells(cells, Some(current), 3, |f| {
            PanelMsg::UpdateDrawingEdit(E::Fill(f))
        }),
        muted,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(kind: &str, info: &[(&str, &str)]) -> Vec<String> {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selection_info = std::iter::once(("Type".to_string(), kind.to_string()))
            .chain(info.iter().map(|(k, v)| (k.to_string(), v.to_string())))
            .collect();
        let view = view_drawing_properties(&ctx, Color::WHITE, Color::WHITE, Color::BLACK);
        crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 3000.0))
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    fn order(texts: &[String], wanted: &[&str]) -> Vec<usize> {
        wanted
            .iter()
            .map(|w| {
                texts
                    .iter()
                    .position(|t| t == w)
                    .unwrap_or_else(|| panic!("{w:?} not in {texts:?}"))
            })
            .collect()
    }

    /// Mockup row 48: Location → Properties → Appearance, paired rows, the Fill
    /// segment reads Solid.
    #[test]
    fn a_rectangle_shows_its_sections_in_order() {
        let shown = texts(
            "Rectangle",
            &[
                ("Position", "1, 2"),
                ("Width", "10"),
                ("Height", "5"),
                ("Border", "0.2"),
            ],
        );
        let at = order(
            &shown,
            &[
                "LOCATION",
                "Position",
                "Size",
                "PROPERTIES",
                "Border",
                "APPEARANCE",
                "Fill",
                "Solid",
            ],
        );
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{shown:?}");
    }

    /// Mockup row 48: an arc keeps Center, Radius, Angles (Start / End on one row)
    /// and Width, and has no Fill.
    #[test]
    fn an_arc_keeps_its_own_fields() {
        let shown = texts(
            "Arc",
            &[
                ("Center", "0, 0"),
                ("Radius", "5"),
                ("Start Angle", "0"),
                ("End Angle", "90"),
                ("Width", "0.2"),
            ],
        );
        let at = order(
            &shown,
            &["Center", "Radius", "Angles", "Start", "End", "Width"],
        );
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{shown:?}");
        assert!(!shown.iter().any(|t| t == "Fill"), "{shown:?}");
    }
}
