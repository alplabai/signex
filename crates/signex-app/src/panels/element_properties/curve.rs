//! Properties of an ellipse, elliptical arc or round rectangle drawing
//! (#1267): the shape's centre, radii, angles or corners as boxes, then the
//! line's width and colour and a closed shape's fill. Laid out as the
//! Bezier's (mockup row 157); no mockup row draws these shapes.

use iced::widget::Column;
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_types::schematic::{CurveShape, FillType, SchCurve, SchDrawing, WireWidthPreset};

use super::super::{PanelContext, PanelMsg};
use super::annotation::{AnnotationField, Rows};
use crate::app::sch_tables::{AnnotationEdit, Axis};
use crate::choice::Choice;

pub(super) fn view_curve_properties<'a>(
    ctx: &'a PanelContext,
    drawing: &SchDrawing,
    c: &SchCurve,
) -> Element<'a, PanelMsg> {
    let rows = Rows::new(ctx, drawing);
    let (icon, title) = match c.shape {
        CurveShape::Ellipse { .. } => (
            crate::icons::icon_dd_ellipse(ctx.theme_id),
            ids::PROPERTIES_ELLIPSE_TITLE,
        ),
        CurveShape::EllipticalArc { .. } => (
            crate::icons::icon_dd_arc(ctx.theme_id),
            ids::PROPERTIES_ELLIPTICAL_ARC_TITLE,
        ),
        CurveShape::RoundRect { .. } => (
            crate::icons::icon_dd_round_rect(ctx.theme_id),
            ids::PROPERTIES_ROUND_RECT_TITLE,
        ),
        CurveShape::Pie { .. } => (
            crate::icons::icon_dd_arc(ctx.theme_id),
            ids::PROPERTIES_PIE_TITLE,
        ),
    };
    let mut col = Column::new().spacing(0).width(Length::Fill);
    col = col.push(rows.header(icon, rows.t(title), rows.t(ids::PROPERTIES_CURVE_CAPTION)));
    col = col.push(rows.section(
        "curve_shape",
        ids::PROPERTIES_CURVE_SECTION_SHAPE,
        shape_rows(&rows, &c.shape, |y| ctx.shown_y(y)),
    ));
    col = col.push(rows.section(
        "curve_line",
        ids::PROPERTIES_ANNOTATION_SECTION_LINE,
        line_rows(&rows, c),
    ));
    if matches!(
        c.shape,
        CurveShape::EllipticalArc { .. } | CurveShape::Pie { .. }
    ) {
        col = col.push(rows.note(rows.t(ids::PROPERTIES_CURVE_ANGLES_NOTE)));
    }
    col.boxed()
}

/// A length or angle box with its label.
fn number<'a>(
    rows: &Rows<'a>,
    label: MessageId,
    field: AnnotationField,
    value: f64,
) -> Element<'a, PanelMsg> {
    rows.field(rows.t(label), rows.input(field, format!("{value:.2}")))
}

fn shape_rows<'a>(
    rows: &Rows<'a>,
    shape: &CurveShape,
    page_y: impl Fn(f64) -> f64,
) -> Vec<Element<'a, PanelMsg>> {
    let center = |at: signex_types::schematic::Point| {
        rows.point(
            rows.t(ids::PROPERTIES_DRAWING_CENTER),
            (
                AnnotationField::CurveCenter(Axis::X),
                AnnotationField::CurveCenter(Axis::Y),
            ),
            (at.x, page_y(at.y)),
        )
    };
    let radii = |rx: f64, ry: f64| {
        [
            number(
                rows,
                ids::PROPERTIES_CURVE_RADIUS_X,
                AnnotationField::CurveRadius(Axis::X),
                rx,
            ),
            number(
                rows,
                ids::PROPERTIES_CURVE_RADIUS_Y,
                AnnotationField::CurveRadius(Axis::Y),
                ry,
            ),
        ]
    };
    let angles = |start: f64, end: f64| {
        [
            number(
                rows,
                ids::PROPERTIES_ANGLE_START,
                AnnotationField::CurveAngle(false),
                start,
            ),
            number(
                rows,
                ids::PROPERTIES_ANGLE_END,
                AnnotationField::CurveAngle(true),
                end,
            ),
        ]
    };
    match *shape {
        CurveShape::Ellipse {
            center: at,
            radius_x,
            radius_y,
        } => std::iter::once(center(at))
            .chain(radii(radius_x, radius_y))
            .collect(),
        CurveShape::EllipticalArc {
            center: at,
            radius_x,
            radius_y,
            start_deg,
            end_deg,
        } => std::iter::once(center(at))
            .chain(radii(radius_x, radius_y))
            .chain(angles(start_deg, end_deg))
            .collect(),
        CurveShape::Pie {
            center: at,
            radius,
            start_deg,
            end_deg,
        } => vec![
            center(at),
            number(
                rows,
                ids::PROPERTIES_DRAWING_RADIUS,
                AnnotationField::CurveRadius(Axis::X),
                radius,
            ),
        ]
        .into_iter()
        .chain(angles(start_deg, end_deg))
        .collect(),
        CurveShape::RoundRect {
            from,
            to,
            corner_radius,
        } => {
            let corner = |label: MessageId, second: bool, at: signex_types::schematic::Point| {
                rows.point(
                    rows.t(label),
                    (
                        AnnotationField::CurveCorner(second, Axis::X),
                        AnnotationField::CurveCorner(second, Axis::Y),
                    ),
                    (at.x, page_y(at.y)),
                )
            };
            vec![
                corner(ids::PROPERTIES_CURVE_FIRST_CORNER, false, from),
                corner(ids::PROPERTIES_CURVE_OPPOSITE_CORNER, true, to),
                number(
                    rows,
                    ids::PROPERTIES_CURVE_CORNER_RADIUS,
                    AnnotationField::CornerRadius,
                    corner_radius,
                ),
            ]
        }
    }
}

fn line_rows<'a>(rows: &Rows<'a>, c: &SchCurve) -> Vec<Element<'a, PanelMsg>> {
    let ctx = rows.ctx;
    let widths = WireWidthPreset::ALL
        .iter()
        .map(|&p| Choice {
            value: p,
            label: ctx.i18n.message_plain(super::bezier::width_label(p), &[]),
        })
        .collect();
    let colour = if c.stroke_color.is_some() {
        rows.t(ids::PROPERTIES_ANNOTATION_COLOUR_OWN)
    } else {
        rows.t(ids::PROPERTIES_ANNOTATION_COLOUR_SHEET)
    };
    let mut out = vec![
        rows.field(
            rows.t(ids::PROPERTIES_ANNOTATION_WIDTH),
            rows.dropdown(
                widths,
                Some(WireWidthPreset::from_stroke_mm(c.width)),
                |p| AnnotationEdit::Width(p.stroke_mm()),
            ),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_ANNOTATION_COLOUR),
            rows.value(colour),
        ),
    ];
    if c.shape.is_closed() {
        let fills = [
            (FillType::None, ids::PANELS_SHAPE_FILL_NONE),
            (FillType::Outline, ids::PANELS_SHAPE_FILL_OUTLINE),
            (FillType::Background, ids::PROPERTIES_FILL_SOLID),
        ];
        let cells = fills
            .into_iter()
            .map(|(f, label)| (label, c.fill == f, AnnotationEdit::Fill(f)))
            .collect();
        out.push(rows.field(rows.t(ids::PANELS_SHAPE_FILL), rows.segmented(cells)));
    }
    out
}
