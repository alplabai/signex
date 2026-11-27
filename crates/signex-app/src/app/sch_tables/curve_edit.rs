//! One Properties edit of a curve drawing (#1267): an ellipse, elliptical
//! arc or round rectangle. An edit that would leave the shape with no
//! visible size or no sweep is refused; a round rectangle's radius stays at
//! most half its short side.

use signex_types::schematic::{CurveShape, Point, SchCurve};

use super::annotation_edit::{AnnotationEdit, Axis, set_axis};

/// Apply `edit` to `c`; `false` when it does not apply to this shape or is
/// refused.
pub(super) fn apply_to_curve(c: &mut SchCurve, edit: &AnnotationEdit) -> bool {
    match edit {
        AnnotationEdit::Width(w) => {
            c.width = w.max(0.0);
            return true;
        }
        AnnotationEdit::StrokeColor(color) => {
            c.stroke_color = *color;
            return true;
        }
        AnnotationEdit::Fill(fill) => {
            if !c.shape.is_closed() {
                return false;
            }
            c.fill = *fill;
            return true;
        }
        _ => {}
    }
    let mut shape = c.shape;
    if !edit_shape(&mut shape, edit) || shape.is_degenerate() {
        return false;
    }
    c.shape = shape;
    true
}

/// The shape with `edit` applied; `false` when the edit names a field the
/// shape does not have.
fn edit_shape(shape: &mut CurveShape, edit: &AnnotationEdit) -> bool {
    match (&mut *shape, edit) {
        (
            CurveShape::Ellipse { center, .. }
            | CurveShape::EllipticalArc { center, .. }
            | CurveShape::Pie { center, .. },
            AnnotationEdit::CurveCenter(axis, v),
        ) => set_axis(center, *axis, *v),
        (
            CurveShape::Ellipse { radius_x, .. } | CurveShape::EllipticalArc { radius_x, .. },
            AnnotationEdit::CurveRadius(Axis::X, v),
        ) => *radius_x = *v,
        (
            CurveShape::Ellipse { radius_y, .. } | CurveShape::EllipticalArc { radius_y, .. },
            AnnotationEdit::CurveRadius(Axis::Y, v),
        ) => *radius_y = *v,
        (CurveShape::Pie { radius, .. }, AnnotationEdit::CurveRadius(_, v)) => *radius = *v,
        (
            CurveShape::EllipticalArc { start_deg, .. } | CurveShape::Pie { start_deg, .. },
            AnnotationEdit::CurveAngle(false, v),
        ) => *start_deg = v.rem_euclid(360.0),
        (
            CurveShape::EllipticalArc { end_deg, .. } | CurveShape::Pie { end_deg, .. },
            AnnotationEdit::CurveAngle(true, v),
        ) => *end_deg = v.rem_euclid(360.0),
        (CurveShape::RoundRect { from, to, .. }, AnnotationEdit::CurveCorner(second, axis, v)) => {
            let corner = if *second { &mut *to } else { &mut *from };
            set_axis(corner, *axis, *v);
            let (a, b) = (*from, *to);
            *from = Point::new(a.x.min(b.x), a.y.min(b.y));
            *to = Point::new(a.x.max(b.x), a.y.max(b.y));
        }
        (CurveShape::RoundRect { corner_radius, .. }, AnnotationEdit::CornerRadius(v))
            if *v >= 0.0 =>
        {
            *corner_radius = *v;
        }
        _ => return false,
    }
    shape.clamp_corner_radius();
    true
}
