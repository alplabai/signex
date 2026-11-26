//! Properties of a Bezier (#1267): the four points as X/Y boxes, then the
//! line's width, style, colour and arrowheads. The illustration names the
//! control points C1 / C2 as the canvas does (DECISIONS 2026-11-19).

use iced::widget::Column;
use iced::{Element, Length, Point, Rectangle, Size, Widget as _};
use signex_i18n::ids;
use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{LineEnds, SchBezier, WireWidthPreset};
use signex_widgets::property_illustration::{
    Anchor, Illustration, Part, Primitive, Style, TextLabel, TextRole, Weight,
};

use super::super::{PanelContext, PanelMsg};
use super::annotation::{AnnotationField, Rows};
use crate::app::sch_tables::{AnnotationEdit, Axis};
use crate::choice::Choice;

const POINT_LABELS: [signex_i18n::MessageId; 4] = [
    ids::PROPERTIES_BEZIER_START,
    ids::PROPERTIES_BEZIER_CONTROL_1,
    ids::PROPERTIES_BEZIER_CONTROL_2,
    ids::PROPERTIES_BEZIER_END,
];

/// Field ids linking the illustration to the rows.
const FIELD_KEYS: [&str; 4] = [
    "bezier.start",
    "bezier.control1",
    "bezier.control2",
    "bezier.end",
];

pub(super) fn view_bezier_properties<'a>(
    ctx: &'a PanelContext,
    drawing: &signex_types::schematic::SchDrawing,
    b: &SchBezier,
    page_y: impl Fn(f64) -> f64,
) -> Element<'a, PanelMsg> {
    let rows = Rows::new(ctx, drawing);
    let mut col = Column::new().spacing(0).width(Length::Fill);
    col = col.push(rows.header(
        crate::icons::icon_dd_bezier(ctx.theme_id),
        rows.t(ids::PROPERTIES_BEZIER_TITLE),
        rows.t(ids::PROPERTIES_BEZIER_CAPTION),
    ));
    col = col.push(rows.illustration(illustration(&rows, b)));

    let points: Vec<Element<'a, PanelMsg>> = b
        .points()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let i = i as u8;
            rows.point(
                rows.t(POINT_LABELS[usize::from(i)]),
                (
                    AnnotationField::BezierPoint(i, Axis::X),
                    AnnotationField::BezierPoint(i, Axis::Y),
                ),
                (p.x, page_y(p.y)),
            )
        })
        .collect();
    col = col.push(rows.section(
        "bezier_points",
        ids::PROPERTIES_ANNOTATION_SECTION_POINTS,
        points,
    ));

    let widths = WireWidthPreset::ALL
        .iter()
        .map(|&p| Choice {
            value: p,
            label: ctx.i18n.message_plain(width_label(p), &[]),
        })
        .collect();
    let line = vec![
        rows.field(
            rows.t(ids::PROPERTIES_ANNOTATION_WIDTH),
            rows.dropdown(
                widths,
                Some(WireWidthPreset::from_stroke_mm(b.width)),
                |p| AnnotationEdit::Width(p.stroke_mm()),
            ),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_ANNOTATION_STYLE),
            rows.segmented(
                [
                    (NetLineStyle::Solid, ids::PROPERTIES_ANNOTATION_STYLE_SOLID),
                    (NetLineStyle::Dashed, ids::PROPERTIES_ANNOTATION_STYLE_DASH),
                    (NetLineStyle::Dotted, ids::PROPERTIES_ANNOTATION_STYLE_DOT),
                ]
                .into_iter()
                .map(|(s, label)| (label, b.line_style == s, AnnotationEdit::LineStyle(s)))
                .collect(),
            ),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_ANNOTATION_COLOUR),
            colour_value(&rows, b),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_ANNOTATION_ENDS),
            rows.dropdown(
                Choice::named(&LineEnds::ALL, ends_label, &ctx.i18n),
                Some(b.ends),
                AnnotationEdit::Ends,
            ),
        ),
    ];
    col = col.push(rows.section("bezier_line", ids::PROPERTIES_ANNOTATION_SECTION_LINE, line));
    col = col.push(rows.note(rows.t(ids::PROPERTIES_BEZIER_NOTE)));
    col.boxed()
}

pub(super) fn width_label(p: WireWidthPreset) -> signex_i18n::MessageId {
    match p {
        WireWidthPreset::Smallest => ids::PROPERTIES_WIRE_WIDTH_SMALLEST,
        WireWidthPreset::Small => ids::PROPERTIES_WIRE_WIDTH_SMALL,
        WireWidthPreset::Medium => ids::PROPERTIES_WIRE_WIDTH_MEDIUM,
        WireWidthPreset::Large => ids::PROPERTIES_WIRE_WIDTH_LARGE,
    }
}

fn ends_label(e: LineEnds) -> signex_i18n::MessageId {
    match e {
        LineEnds::None => ids::PROPERTIES_ANNOTATION_ENDS_NONE,
        LineEnds::Start => ids::PROPERTIES_ANNOTATION_ENDS_START,
        LineEnds::End => ids::PROPERTIES_ANNOTATION_ENDS_END,
        LineEnds::Both => ids::PROPERTIES_ANNOTATION_ENDS_BOTH,
    }
}

/// The colour row: the sheet's drawing colour unless the curve has its own.
fn colour_value<'a>(rows: &Rows<'a>, b: &SchBezier) -> Element<'a, PanelMsg> {
    let caption = if b.stroke_color.is_some() {
        rows.t(ids::PROPERTIES_ANNOTATION_COLOUR_OWN)
    } else {
        rows.t(ids::PROPERTIES_ANNOTATION_COLOUR_SHEET)
    };
    rows.value(caption)
}

/// The curve, its control lines and four points, fitted to the view box.
fn illustration(rows: &Rows<'_>, b: &SchBezier) -> Illustration {
    const W: f32 = 240.0;
    const H: f32 = 120.0;
    const PAD: f32 = 24.0;
    let pts = b.flatten();
    let (lo, hi) = b.bounds();
    let ctrl = b.points();
    let min_x = ctrl.iter().map(|p| p.x).fold(lo.x, f64::min);
    let min_y = ctrl.iter().map(|p| p.y).fold(lo.y, f64::min);
    let max_x = ctrl.iter().map(|p| p.x).fold(hi.x, f64::max);
    let max_y = ctrl.iter().map(|p| p.y).fold(hi.y, f64::max);
    let span = ((max_x - min_x).max(max_y - min_y)).max(1e-6) as f32;
    let scale = ((W - 2.0 * PAD) / span).min((H - 2.0 * PAD) / span);
    let at = |p: signex_types::schematic::Point| {
        Point::new(
            PAD + (p.x - min_x) as f32 * scale,
            PAD + (p.y - min_y) as f32 * scale,
        )
    };
    let [s, c1, c2, e] = ctrl.map(at);
    let mut art = Illustration::new(W, H)
        .part(Part::new(
            Primitive::Line { from: s, to: c1 },
            Style::dashed(),
        ))
        .part(Part::new(
            Primitive::Line { from: e, to: c2 },
            Style::dashed(),
        ))
        .part(Part::new(
            Primitive::Polyline {
                points: pts.iter().map(|p| at(*p)).collect(),
                is_closed: false,
            },
            Style::outline(
                signex_widgets::property_illustration::Ink::Secondary,
                Weight::Heavy,
            ),
        ));
    let square = |c: Point| Rectangle::new(Point::new(c.x - 2.5, c.y - 2.5), Size::new(5.0, 5.0));
    for (i, p) in [s, c1, c2, e].into_iter().enumerate() {
        let mark = if i == 0 || i == 3 {
            Primitive::Rect { rect: square(p) }
        } else {
            Primitive::Circle {
                center: p,
                radius: 2.5,
            }
        };
        art = art.part(Part::new(mark, Style::edge()).linked(FIELD_KEYS[i]));
    }
    let label = |at: Point, id, field: &'static str| {
        Part::label(
            TextLabel {
                at: Point::new(at.x, at.y - 8.0),
                text: rows.t(id).into(),
                anchor: Anchor::Middle,
                role: TextRole::Caption,
            },
            signex_widgets::property_illustration::Ink::Faint,
        )
        .linked(field)
    };
    art.part(label(s, ids::PROPERTIES_BEZIER_START, FIELD_KEYS[0]))
        .part(label(c1, ids::PROPERTIES_BEZIER_C1, FIELD_KEYS[1]))
        .part(label(c2, ids::PROPERTIES_BEZIER_C2, FIELD_KEYS[2]))
        .part(label(e, ids::PROPERTIES_BEZIER_END, FIELD_KEYS[3]))
}
