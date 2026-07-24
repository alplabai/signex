use crate::transmission_line_calculator::Complex;

use super::smith_sphere_point::SmithSpherePoint;

/// Lifts a reflection coefficient from the disk onto the Smith sphere.
pub(super) fn reflection_to_smith_sphere(reflection_coefficient: Complex) -> SmithSpherePoint {
    let magnitude_squared = reflection_coefficient.re.mul_add(
        reflection_coefficient.re,
        reflection_coefficient.im * reflection_coefficient.im,
    );
    let denominator = 1.0 + magnitude_squared;

    SmithSpherePoint::new(
        2.0 * reflection_coefficient.re / denominator,
        2.0 * reflection_coefficient.im / denominator,
        (1.0 - magnitude_squared) / denominator,
    )
}

/// Maps normalized impedance directly onto the Smith sphere.
pub(super) fn normalized_impedance_to_smith_sphere(
    normalized_impedance: Complex,
) -> SmithSpherePoint {
    let magnitude_squared = normalized_impedance.re.mul_add(
        normalized_impedance.re,
        normalized_impedance.im * normalized_impedance.im,
    );
    let denominator = magnitude_squared + 1.0;

    SmithSpherePoint::new(
        (magnitude_squared - 1.0) / denominator,
        2.0 * normalized_impedance.im / denominator,
        2.0 * normalized_impedance.re / denominator,
    )
}
