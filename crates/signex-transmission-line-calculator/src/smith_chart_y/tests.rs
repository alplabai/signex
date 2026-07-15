use super::*;

#[test]
fn matched_admittance_maps_to_the_chart_center() {
    assert_eq!(admittance_chart_point(Complex::ONE), (0.0, 0.0));
}

#[test]
fn high_conductance_maps_towards_the_left_side() {
    let (x, y) = admittance_chart_point(Complex::new(1000.0, 0.0));

    assert!(x < -0.99);
    assert!(y.abs() < f64::EPSILON);
}
