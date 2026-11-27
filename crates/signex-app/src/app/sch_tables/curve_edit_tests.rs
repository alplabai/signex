//! Properties edits of a curve drawing (#1267): geometry, line and fill,
//! and the edits a shape refuses.

use signex_types::schematic::{CurveShape, FillType, Point, SchCurve, SchDrawing};

use super::{AnnotationEdit, Axis, apply_annotation_edit};

fn drawing(shape: CurveShape) -> SchDrawing {
    SchDrawing::Curve(SchCurve::new(uuid::Uuid::from_u128(7), shape, 0.0))
}

fn shape_of(d: &SchDrawing) -> CurveShape {
    match d {
        SchDrawing::Curve(c) => c.shape,
        other => panic!("not a curve: {other:?}"),
    }
}

fn ellipse() -> CurveShape {
    CurveShape::Ellipse {
        center: Point::new(10.0, 10.0),
        radius_x: 5.0,
        radius_y: 2.5,
    }
}

fn arc() -> CurveShape {
    CurveShape::EllipticalArc {
        center: Point::new(0.0, 0.0),
        radius_x: 5.0,
        radius_y: 2.5,
        start_deg: 0.0,
        end_deg: 90.0,
    }
}

#[test]
fn an_ellipse_takes_its_centre_and_radii() {
    let d = drawing(ellipse());
    let moved = apply_annotation_edit(&d, &AnnotationEdit::CurveCenter(Axis::Y, 12.7)).unwrap();
    let wider = apply_annotation_edit(&moved, &AnnotationEdit::CurveRadius(Axis::X, 7.62)).unwrap();
    assert_eq!(
        shape_of(&wider),
        CurveShape::Ellipse {
            center: Point::new(10.0, 12.7),
            radius_x: 7.62,
            radius_y: 2.5,
        }
    );
}

#[test]
fn an_arc_angle_is_kept_in_a_turn_and_an_empty_sweep_is_refused() {
    let d = drawing(arc());
    let next = apply_annotation_edit(&d, &AnnotationEdit::CurveAngle(true, -90.0)).unwrap();
    let CurveShape::EllipticalArc { end_deg, .. } = shape_of(&next) else {
        panic!("an arc");
    };
    assert_eq!(end_deg, 270.0);
    assert!(apply_annotation_edit(&d, &AnnotationEdit::CurveAngle(true, 0.0)).is_none());
}

#[test]
fn a_radius_too_small_to_see_is_refused() {
    let d = drawing(ellipse());
    for v in [0.0, -1.0, 0.001] {
        assert!(apply_annotation_edit(&d, &AnnotationEdit::CurveRadius(Axis::X, v)).is_none());
    }
}

#[test]
fn a_round_rect_corner_keeps_from_minimal_and_clamps_the_radius() {
    let d = drawing(CurveShape::round_rect(
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
    ));
    // The first corner dragged past the other: the two swap.
    let next =
        apply_annotation_edit(&d, &AnnotationEdit::CurveCorner(false, Axis::X, 20.0)).unwrap();
    let CurveShape::RoundRect { from, to, .. } = shape_of(&next) else {
        panic!("a round rectangle");
    };
    assert_eq!((from, to), (Point::new(10.0, 0.0), Point::new(20.0, 10.0)));

    let big = apply_annotation_edit(&d, &AnnotationEdit::CornerRadius(50.0)).unwrap();
    let CurveShape::RoundRect { corner_radius, .. } = shape_of(&big) else {
        panic!("a round rectangle");
    };
    assert_eq!(corner_radius, 5.0, "half the short side");
    assert!(apply_annotation_edit(&d, &AnnotationEdit::CornerRadius(-1.0)).is_none());
}

#[test]
fn a_closed_curve_takes_a_fill_and_an_arc_refuses_one() {
    let filled = apply_annotation_edit(
        &drawing(ellipse()),
        &AnnotationEdit::Fill(FillType::Background),
    )
    .unwrap();
    let SchDrawing::Curve(c) = filled else {
        panic!("a curve");
    };
    assert_eq!(c.fill, FillType::Background);
    assert!(
        apply_annotation_edit(&drawing(arc()), &AnnotationEdit::Fill(FillType::Background))
            .is_none()
    );
}

#[test]
fn width_and_colour_apply_and_a_field_the_shape_lacks_does_not() {
    let d = drawing(ellipse());
    let SchDrawing::Curve(c) = apply_annotation_edit(&d, &AnnotationEdit::Width(0.5)).unwrap()
    else {
        panic!("a curve");
    };
    assert_eq!(c.width, 0.5);
    assert!(apply_annotation_edit(&d, &AnnotationEdit::CornerRadius(1.0)).is_none());
    assert!(apply_annotation_edit(&d, &AnnotationEdit::CurveAngle(false, 10.0)).is_none());
}
