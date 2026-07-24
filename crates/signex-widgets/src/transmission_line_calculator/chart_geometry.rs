use crate::transmission_line_calculator::rust_rf_adapter::{
    impedance_from_reflection as rust_rf_impedance_from_reflection,
    reflection_coefficient as rust_rf_reflection_coefficient,
};
use crate::transmission_line_calculator::*;

pub(crate) const SPEED_OF_LIGHT_M_PER_S: f64 = 299_792_458.0;

/// Maps normalized impedance to reflection coordinates in the Smith-chart disk.
pub fn chart_point_from_normalized_impedance(normalized_impedance: Complex) -> (f64, f64) {
    let gamma = impedance_to_reflection(normalized_impedance, 1.0);
    (gamma.re, gamma.im)
}

/// Maps a point in the Smith-chart disk back to normalized impedance.
pub fn normalized_impedance_from_chart_point(x: f64, y: f64) -> Complex {
    reflection_to_impedance(Complex::new(x, y), 1.0)
}

/// Converts a physical, wavelength, or degree length into meters.
pub fn length_to_meters(
    value: f64,
    unit: ScalarUnit,
    frequency_hz: f64,
    effective_dielectric: f64,
) -> Result<f64, SolveError> {
    if unit == ScalarUnit::Wavelength || unit == ScalarUnit::Degree {
        let wavelengths = if unit == ScalarUnit::Degree {
            value / 360.0
        } else {
            value
        };
        return Ok(wavelengths * SPEED_OF_LIGHT_M_PER_S
            / frequency_hz
            / effective_dielectric.sqrt());
    }
    Ok(value * unit.multiplier())
}

/// Converts impedance to a reflection coefficient for the given reference impedance.
pub fn impedance_to_reflection(impedance: Complex, reference_impedance_ohm: f64) -> Complex {
    rust_rf_reflection_coefficient(impedance, reference_impedance_ohm)
}

/// Converts a reflection coefficient to impedance for the given reference impedance.
pub fn reflection_to_impedance(gamma: Complex, reference_impedance_ohm: f64) -> Complex {
    rust_rf_impedance_from_reflection(gamma, reference_impedance_ohm)
}
