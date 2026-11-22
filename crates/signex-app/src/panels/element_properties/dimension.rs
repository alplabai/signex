//! Properties of a dimension (#1267): Measure (From, To, the measured
//! Length read-only, Direction) and Text (Units, Precision, Arrows and an
//! Override text behind its enable header).

use iced::widget::Column;
use iced::{Element, Length, Point, Widget as _};
use signex_i18n::ids;
use signex_types::schematic::{
    DimensionArrows, DimensionDirection, DimensionUnits, SchDimension, SchDrawing,
};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Part, Primitive, Style, TextLabel, TextRole,
};

use super::super::{PanelContext, PanelMsg};
use super::annotation::{AnnotationField, DimensionToolForm, Rows};
use crate::app::sch_tables::{AnnotationEdit, Axis};
use crate::choice::Choice;

const FROM_KEY: &str = "dimension.from";
const TO_KEY: &str = "dimension.to";

pub(super) fn view_dimension_properties<'a>(
    ctx: &'a PanelContext,
    drawing: &SchDrawing,
    d: &SchDimension,
) -> Element<'a, PanelMsg> {
    let rows = Rows::new(ctx, drawing);
    let title = match d.direction {
        DimensionDirection::Aligned => ids::PROPERTIES_DIMENSION_ALIGNED,
        _ => ids::PROPERTIES_DIMENSION_LINEAR,
    };
    let header = rows.header(
        crate::icons::icon_pcb_dimension(ctx.theme_id),
        rows.t(title),
        signex_i18n::UserText::new(d.label()),
    );
    body(&rows, header, d, (true, true))
}

/// The armed tool's form (mockup row 157): the same rows, editing what the
/// next dimension is placed with. A point not clicked yet shows blank.
pub(super) fn view_dimension_tool<'a>(
    ctx: &'a PanelContext,
    form: &DimensionToolForm,
) -> Element<'a, PanelMsg> {
    let d = &form.dimension;
    let rows = Rows::new(ctx, &SchDrawing::Dimension(d.clone()));
    let title = if form.is_aligned {
        ids::PROPERTIES_DIMENSION_ALIGNED
    } else {
        ids::PROPERTIES_DIMENSION_LINEAR
    };
    let header = rows.header(
        crate::icons::icon_pcb_dimension(ctx.theme_id),
        rows.t(title),
        rows.t(ids::PROPERTIES_DIMENSION_TOOL_CAPTION),
    );
    body(&rows, header, d, (form.has_from, form.has_to))
}

/// Header, illustration, Measure and Text; `clicked` says whether From and
/// To hold real points.
fn body<'a>(
    rows: &Rows<'a>,
    header: Element<'a, PanelMsg>,
    d: &SchDimension,
    clicked: (bool, bool),
) -> Element<'a, PanelMsg> {
    Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(header)
        .push(rows.illustration(illustration(rows, d)))
        .push(rows.section(
            "dimension_measure",
            ids::PROPERTIES_DIMENSION_SECTION_MEASURE,
            measure_rows(rows, d, clicked),
        ))
        .push(rows.section(
            "dimension_text",
            ids::PROPERTIES_DIMENSION_SECTION_TEXT,
            text_rows(rows, d, clicked.1),
        ))
        .boxed()
}

fn point_row<'a>(
    rows: &Rows<'a>,
    label: signex_i18n::MessageId,
    second: bool,
    p: signex_types::schematic::Point,
    is_clicked: bool,
) -> Element<'a, PanelMsg> {
    if !is_clicked {
        return rows.blank_point(rows.t(label));
    }
    rows.point(
        rows.t(label),
        (
            AnnotationField::DimensionPoint(second, Axis::X),
            AnnotationField::DimensionPoint(second, Axis::Y),
        ),
        (p.x, rows.ctx.shown_y(p.y)),
    )
}

fn measure_rows<'a>(
    rows: &Rows<'a>,
    d: &SchDimension,
    (has_from, has_to): (bool, bool),
) -> Vec<Element<'a, PanelMsg>> {
    let mm = rows
        .ctx
        .i18n
        .message_plain(ids::PROPERTIES_ANNOTATION_MM, &[]);
    let length = if has_to {
        format!("{:.3} {mm}", d.measured_mm())
    } else {
        mm.to_string()
    };
    vec![
        point_row(
            rows,
            ids::PROPERTIES_DIMENSION_FROM,
            false,
            d.from,
            has_from,
        ),
        point_row(rows, ids::PROPERTIES_DIMENSION_TO, true, d.to, has_to),
        rows.field(
            rows.t(ids::PROPERTIES_DIMENSION_LENGTH),
            rows.value(signex_i18n::UserText::new(length)),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_DIMENSION_DIRECTION),
            rows.segmented(
                [
                    (
                        DimensionDirection::Horizontal,
                        ids::PROPERTIES_DIMENSION_HORIZONTAL,
                    ),
                    (
                        DimensionDirection::Vertical,
                        ids::PROPERTIES_DIMENSION_VERTICAL,
                    ),
                    (
                        DimensionDirection::Aligned,
                        ids::PROPERTIES_DIMENSION_ALIGNED_DIRECTION,
                    ),
                ]
                .into_iter()
                .map(|(v, label)| (label, d.direction == v, AnnotationEdit::Direction(v)))
                .collect(),
            ),
        ),
    ]
}

fn text_rows<'a>(
    rows: &Rows<'a>,
    d: &SchDimension,
    has_measure: bool,
) -> Vec<Element<'a, PanelMsg>> {
    let ctx = rows.ctx;
    let precision_ids = [
        ids::PROPERTIES_DIMENSION_PRECISION_0,
        ids::PROPERTIES_DIMENSION_PRECISION_1,
        ids::PROPERTIES_DIMENSION_PRECISION_2,
        ids::PROPERTIES_DIMENSION_PRECISION_3,
    ];
    vec![
        rows.field(
            rows.t(ids::PROPERTIES_DIMENSION_UNITS),
            rows.segmented(
                [
                    (DimensionUnits::Mm, ids::PROPERTIES_DIMENSION_UNIT_MM),
                    (DimensionUnits::Mil, ids::PROPERTIES_DIMENSION_UNIT_MIL),
                    (DimensionUnits::In, ids::PROPERTIES_DIMENSION_UNIT_IN),
                ]
                .into_iter()
                .map(|(v, label)| (label, d.units == v, AnnotationEdit::Units(v)))
                .collect(),
            ),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_DIMENSION_PRECISION),
            rows.segmented(
                (0u8..=3)
                    .map(|p| {
                        (
                            precision_ids[usize::from(p)],
                            d.precision.min(3) == p,
                            AnnotationEdit::Precision(p),
                        )
                    })
                    .collect(),
            ),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_DIMENSION_ARROWS),
            rows.dropdown(
                Choice::named(&DimensionArrows::ALL, arrows_label, &ctx.i18n),
                Some(d.arrows),
                AnnotationEdit::Arrows,
            ),
        ),
        override_row(rows, d, has_measure),
    ]
}

/// The Override text header, which switches its Text row.
fn override_row<'a>(rows: &Rows<'a>, d: &SchDimension, has_measure: bool) -> Element<'a, PanelMsg> {
    let shown = d.override_text.clone().unwrap_or_else(|| {
        if has_measure {
            d.measured_text()
        } else {
            String::new()
        }
    });
    signex_widgets::enable_section::EnableSection::new(
        rows.t(ids::PROPERTIES_DIMENSION_OVERRIDE),
        d.override_text.is_some(),
    )
    .on_toggle(|on| super::annotation::picked(AnnotationEdit::OverrideEnabled(on)))
    .view(
        crate::panels::enable_section::enable_section_style(&rows.ctx.tokens),
        rows.field(
            rows.t(ids::PROPERTIES_DIMENSION_OVERRIDE_TEXT),
            rows.input(AnnotationField::OverrideText, shown),
        ),
    )
}

fn arrows_label(a: DimensionArrows) -> signex_i18n::MessageId {
    match a {
        DimensionArrows::FilledInside => ids::PROPERTIES_DIMENSION_ARROWS_INSIDE,
        DimensionArrows::FilledOutside => ids::PROPERTIES_DIMENSION_ARROWS_OUTSIDE,
        DimensionArrows::Open => ids::PROPERTIES_DIMENSION_ARROWS_OPEN,
    }
}

/// The measured span: its two points, extension lines and the dimension
/// line with the value; From / To named at the outer ends of the line.
fn illustration(rows: &Rows<'_>, d: &SchDimension) -> Illustration {
    const W: f32 = 240.0;
    const H: f32 = 110.0;
    const PAD: f32 = 34.0;
    let g = d.geometry();
    let all = d.outline_points();
    let min_x = all.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let min_y = all.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_x = all.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let max_y = all.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let span = ((max_x - min_x).max(max_y - min_y)).max(1e-6) as f32;
    let scale = ((W - 2.0 * PAD) / span).min((H - 2.0 * PAD) / span);
    let at = |p: signex_types::schematic::Point| {
        Point::new(
            PAD + (p.x - min_x) as f32 * scale,
            PAD + (p.y - min_y) as f32 * scale,
        )
    };
    let (from, to) = (at(d.from), at(d.to));
    let (a, b) = (at(g.line[0]), at(g.line[1]));
    let callout = Callout::new(a, b)
        .label(signex_i18n::UserText::new(d.label()))
        .extension(from, a)
        .extension(to, b);
    let end_label = |p: Point, toward: Point, id, field: &'static str| {
        let dx = p.x - toward.x;
        let anchor = if dx >= 0.0 {
            Anchor::Start
        } else {
            Anchor::End
        };
        Part::label(
            TextLabel {
                at: Point::new(p.x + dx.signum() * 6.0, p.y + 3.0),
                text: rows.t(id).into(),
                anchor,
                role: TextRole::Caption,
            },
            Ink::Faint,
        )
        .linked(field)
    };
    Illustration::new(W, H)
        .part(
            Part::new(
                Primitive::Circle {
                    center: from,
                    radius: 2.5,
                },
                Style::edge(),
            )
            .linked(FROM_KEY),
        )
        .part(
            Part::new(
                Primitive::Circle {
                    center: to,
                    radius: 2.5,
                },
                Style::edge(),
            )
            .linked(TO_KEY),
        )
        .part(Part::callout(callout).linked(FROM_KEY).linked(TO_KEY))
        .part(end_label(a, b, ids::PROPERTIES_DIMENSION_FROM, FROM_KEY))
        .part(end_label(b, a, ids::PROPERTIES_DIMENSION_TO, TO_KEY))
}
