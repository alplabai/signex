use super::*;

/// Verifies that pan is measured from gesture start and ends cleanly.
#[test]
fn pan_is_measured_from_gesture_start_and_ends_cleanly() {
    let mut navigation = SmithViewNavigationState::default();
    let transform = SmithViewTransform::identity().zoomed_at(Point::ORIGIN, 2.0);
    navigation.begin_pan(Point::new(20.0, 30.0), transform);

    let panned = navigation.pan_to(Point::new(37.0, 24.0));

    assert_eq!(panned.translation(), Vector::new(17.0, -6.0));
    assert!(navigation.end_pan());
    assert!(!navigation.is_panning());
    assert!(!navigation.end_pan());
}
