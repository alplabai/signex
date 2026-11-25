//! The dimension illustration (#2523, mockup row 101): the two points it measures,
//! the extension lines out to the dimension line, the Offset, and the
//! measured value on the line; a radial one (#2507) as its leader from the
//! centre with no extension lines and no Offset. A selection whose points
//! differ, or a dimension not placed yet, is drawn as a 10 mm one.

use iced::Point;
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Part, Primitive, Style, TextRole, Weight,
};

use signex_types::pcb_dimension_kind::DimensionKind;

use super::super::{DimensionFacts, format_number};
use super::{VIEW_H, VIEW_W, canvas, cross, label};

pub(in crate::panels::pcb_primitive_properties) const START: FieldId = FieldId::new("dim.start");
pub(in crate::panels::pcb_primitive_properties) const END: FieldId = FieldId::new("dim.end");
pub(in crate::panels::pcb_primitive_properties) const OFFSET: FieldId = FieldId::new("dim.offset");
pub(in crate::panels::pcb_primitive_properties) const VALUE: FieldId = FieldId::new("dim.value");
pub(in crate::panels::pcb_primitive_properties) const STYLE: FieldId = FieldId::new("dim.style");
pub(in crate::panels::pcb_primitive_properties) const LAYER: FieldId = FieldId::new("dim.layer");

/// Where the dimension is fitted: left of it room for the Offset callout,
/// under it room for the note.
const FIT_LEFT: f32 = 70.0;
const FIT_TOP: f32 = 30.0;
const FIT_RIGHT_INSET: f32 = 24.0;
const FIT_BOTTOM_INSET: f32 = 36.0;
/// How far left of the start point the Offset callout runs.
const OFFSET_STANDOFF: f32 = 12.0;

/// The illustration of the selected dimensions `f`.
pub(in crate::panels::pcb_primitive_properties) fn illustration(
    f: &DimensionFacts,
    i18n: &I18n,
) -> Illustration {
    let (start, end) = match (f.start, f.end) {
        ((Some(ax), Some(ay)), (Some(bx), Some(by))) if (ax, ay) != (bx, by) => {
            ((ax, ay), (bx, by))
        }
        _ => ((0.0, 0.0), (10.0, 0.0)),
    };
    // #2507: a radial dimension is a leader from the centre to the circle,
    // with no offset and no extension lines.
    let is_radial = f.kind == Some(DimensionKind::Radial);
    let offset = f
        .offset
        .filter(|o| o.is_finite() && !is_radial)
        .unwrap_or(0.0);
    let [s1, s2, d1, d2] = fitted(start, end, offset);
    let value = f.value.clone().unwrap_or_default();
    let note = t!(
        i18n,
        ids::PCB_PRIM_DIM_DRAWING_NOTE,
        "layer" => f.layer.as_ref().map_or_else(String::new, |l| l.label.clone()),
        "height" => f.text_height.map(format_number).unwrap_or_default()
    );
    let extension = |from: Point, to: Point| {
        Part::new(
            Primitive::Line { from, to },
            Style::outline(Ink::Tertiary, Weight::Thin),
        )
        .linked(OFFSET)
    };
    let mut art = canvas()
        .parts(cross(s1, 4.0, &[START]))
        .parts(cross(s2, 4.0, &[END]));
    if !is_radial {
        art = art.part(extension(s1, d1)).part(extension(s2, d2));
    }
    art = art
        .part(
            Part::callout(Callout::new(d1, d2).label(UserText::new(value)))
                .linked_all([VALUE, STYLE, LAYER]),
        )
        .part(point_label(s1, t!(i18n, ids::PCB_PRIM_POINT_START), START))
        .part(point_label(s2, t!(i18n, ids::PCB_PRIM_POINT_END), END))
        .part(label(
            Point::new(VIEW_W - 6.0, VIEW_H - 8.0),
            note,
            Anchor::End,
            TextRole::Note,
            &[LAYER, STYLE],
        ));
    // No Offset callout for a line drawn on its points.
    if (s1.y - d1.y).abs() > 0.5 || (s1.x - d1.x).abs() > 0.5 {
        let from = Point::new(s1.x - OFFSET_STANDOFF, s1.y);
        let to = Point::new(d1.x - OFFSET_STANDOFF, d1.y);
        art = art.part(
            Part::callout(
                Callout::new(from, to)
                    .label(t!(i18n, ids::PCB_PRIM_OFFSET))
                    .label_at(
                        Point::new(from.x.min(to.x) - 6.0, (from.y + to.y) / 2.0),
                        Anchor::End,
                    ),
            )
            .linked(OFFSET),
        );
    }
    art
}

/// The start, end and the two ends of the dimension line, in view units:
/// the board's Y up turned to the view's Y down and fitted to the view.
fn fitted(start: (f64, f64), end: (f64, f64), offset: f64) -> [Point; 4] {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let len = dx.hypot(dy);
    let (ux, uy) = if len > 0.0 {
        (dx / len, dy / len)
    } else {
        (1.0, 0.0)
    };
    let (nx, ny) = (-uy * offset, ux * offset);
    let board = [
        start,
        end,
        (start.0 + nx, start.1 + ny),
        (end.0 + nx, end.1 + ny),
    ];
    let min_x = board.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = board.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = board.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = board.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let room_w = f64::from(VIEW_W - FIT_RIGHT_INSET - FIT_LEFT);
    let room_h = f64::from(VIEW_H - FIT_BOTTOM_INSET - FIT_TOP);
    let span_w = (max_x - min_x).max(f64::EPSILON);
    let span_h = (max_y - min_y).max(f64::EPSILON);
    let scale = (room_w / span_w).min(room_h / span_h);
    let left = f64::from(FIT_LEFT) + (room_w - (max_x - min_x) * scale) / 2.0;
    let top = f64::from(FIT_TOP) + (room_h - (max_y - min_y) * scale) / 2.0;
    board.map(|(x, y)| {
        Point::new(
            (left + (x - min_x) * scale) as f32,
            (top + (max_y - y) * scale) as f32,
        )
    })
}

/// "Start" or "End" over its point.
fn point_label(at: Point, text: signex_i18n::Localized, field: FieldId) -> Part {
    label(
        Point::new(at.x, at.y - 9.0),
        text,
        Anchor::Middle,
        TextRole::Note,
        &[field],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dimension_fits_the_view_with_the_line_off_its_points() {
        let [s1, s2, d1, d2] = fitted((0.0, 0.0), (62.0, 0.0), -4.0);
        for p in [s1, s2, d1, d2] {
            assert!(
                (0.0..=VIEW_W).contains(&p.x) && (0.0..=VIEW_H).contains(&p.y),
                "{p:?}"
            );
        }
        assert!(s2.x > s1.x, "start left of end");
        // A negative offset of a left-to-right dimension stands below it,
        // which the view draws lower down.
        assert!(d1.y > s1.y && d2.y > s2.y);
    }

    fn facts(kind: DimensionKind) -> DimensionFacts {
        use crate::panels::pcb_primitive_properties::drawing::fixture;
        DimensionFacts {
            common: fixture::common(vec![pcb_geom::ItemRef::Dimension(uuid::Uuid::from_u128(1))]),
            kind: Some(kind),
            start: (Some(0.0), Some(0.0)),
            end: (Some(5.0), Some(0.0)),
            offset: Some(-4.0),
            line_width: Some(0.1),
            text_height: Some(1.0),
            precision: Some(2),
            value: Some("R5.00mm".into()),
            layer: None,
            layers: Vec::new(),
            style: Default::default(),
        }
    }

    /// #2507: a radial dimension is drawn as its leader, with no extension
    /// lines and no Offset callout, whatever offset it holds.
    #[test]
    fn a_radial_dimension_draws_no_extension_lines_and_no_offset() {
        let i18n = I18n::en_us_ref();
        let offset_parts = |kind| {
            illustration(&facts(kind), i18n)
                .visible_parts()
                .filter(|part| part.is_linked_to(&OFFSET))
                .count()
        };
        assert_eq!(offset_parts(DimensionKind::Radial), 0);
        assert_eq!(
            offset_parts(DimensionKind::Aligned),
            3,
            "two extension lines and the Offset callout"
        );
    }

    #[test]
    fn a_vertical_dimension_fits_too() {
        let [s1, s2, ..] = fitted((5.0, 0.0), (5.0, 30.0), 2.0);
        assert!((s1.x - s2.x).abs() < 1e-3);
        assert!(s2.y < s1.y, "the end, higher on the board, is higher up");
    }
}
