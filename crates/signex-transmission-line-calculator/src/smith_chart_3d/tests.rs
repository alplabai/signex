use super::smith_sphere::{normalized_impedance_to_smith_sphere, reflection_to_smith_sphere};
use super::smith_sphere_point::SmithSpherePoint;
use crate::Complex;

fn assert_close(left: f64, right: f64) {
    assert!(
        (left - right).abs() < 1.0e-9,
        "expected {left} to be close to {right}"
    );
}

#[test]
fn matched_load_maps_to_north_pole() {
    let point = normalized_impedance_to_smith_sphere(Complex::ONE);

    assert_close(point.x, SmithSpherePoint::NORTH_POLE.x);
    assert_close(point.y, SmithSpherePoint::NORTH_POLE.y);
    assert_close(point.z, SmithSpherePoint::NORTH_POLE.z);
}

#[test]
fn unit_magnitude_reflection_maps_to_equator() {
    let point = reflection_to_smith_sphere(Complex::new(0.0, 1.0));

    assert_close(point.x, 0.0);
    assert_close(point.y, 1.0);
    assert_close(point.z, 0.0);
}

#[test]
fn large_reflection_approaches_south_pole() {
    let point = reflection_to_smith_sphere(Complex::new(1.0e9, 0.0));

    assert!(point.z < -0.999_999_999);
    assert!(point.x.abs() < 1.0e-8);
    assert!(point.y.abs() < 1.0e-8);
}

#[test]
fn negative_resistance_maps_to_lower_hemisphere() {
    let point = normalized_impedance_to_smith_sphere(Complex::new(-0.5, 0.0));

    assert!(point.z < 0.0);
    assert_close(point.x, -0.6);
    assert_close(point.y, 0.0);
    assert_close(point.z, -0.8);
}

#[test]
fn negative_matched_resistance_maps_to_south_pole() {
    let point = normalized_impedance_to_smith_sphere(Complex::new(-1.0, 0.0));

    assert_close(point.x, SmithSpherePoint::SOUTH_POLE.x);
    assert_close(point.y, SmithSpherePoint::SOUTH_POLE.y);
    assert_close(point.z, SmithSpherePoint::SOUTH_POLE.z);
}
