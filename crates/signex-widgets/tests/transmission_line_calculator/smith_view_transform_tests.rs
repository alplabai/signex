use super::*;

const TOLERANCE: f32 = 1.0e-4;

/// Verifies that inverse reverses composed pan and zoom.
#[test]
fn inverse_reverses_composed_pan_and_zoom() {
    let point = Point::new(17.0, -8.0);
    let transform = SmithViewTransform::identity()
        .translated(Vector::new(24.0, -11.0))
        .zoomed_at(Point::new(80.0, 45.0), 2.5);

    let transformed = transform.transform_point(point);
    let restored = transform.inverse_transform_point(transformed).unwrap();

    assert!((restored.x - point.x).abs() < TOLERANCE);
    assert!((restored.y - point.y).abs() < TOLERANCE);
}

/// Verifies that zoom keeps anchor fixed.
#[test]
fn zoom_keeps_anchor_fixed() {
    let anchor = Point::new(140.0, 90.0);
    let transform = SmithViewTransform::identity()
        .translated(Vector::new(35.0, -12.0))
        .zoomed_at(anchor, 1.8);
    let original_point = SmithViewTransform::identity()
        .translated(Vector::new(35.0, -12.0))
        .inverse_transform_point(anchor)
        .unwrap();

    let transformed_anchor = transform.transform_point(original_point);

    assert!((transformed_anchor.x - anchor.x).abs() < TOLERANCE);
    assert!((transformed_anchor.y - anchor.y).abs() < TOLERANCE);
}

/// Verifies that pan uses screen space delta.
#[test]
fn pan_uses_screen_space_delta() {
    let transform = SmithViewTransform::identity()
        .zoomed_at(Point::ORIGIN, 3.0)
        .translated(Vector::new(18.0, -7.0));

    assert_eq!(transform.translation(), Vector::new(18.0, -7.0));
    assert_eq!(
        transform.transform_point(Point::new(2.0, 4.0)),
        Point::new(24.0, 5.0)
    );
}

/// Verifies that zoom scale is finite and clamped.
#[test]
fn zoom_scale_is_finite_and_clamped() {
    let maximum = SmithViewTransform::identity().zoomed_at(Point::ORIGIN, f32::MAX);
    let minimum = SmithViewTransform::identity().zoomed_at(Point::ORIGIN, f32::MIN_POSITIVE);

    assert_eq!(maximum.scale(), SmithViewTransform::MAXIMUM_SCALE);
    assert_eq!(minimum.scale(), SmithViewTransform::MINIMUM_SCALE);
    assert!(maximum.scale().is_finite());
    assert!(minimum.scale().is_finite());
}
