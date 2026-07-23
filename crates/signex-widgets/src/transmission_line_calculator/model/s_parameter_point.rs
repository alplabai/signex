use crate::transmission_line_calculator::{Complex, SParameterMatrix};
use serde::{Deserialize, Serialize};

/// Stores one frequency sample of a one-port or two-port S-parameter data set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SParameterPoint {
    pub frequency_hz: f64,
    pub s11: Complex,
    pub s21: Option<Complex>,
    pub s12: Option<Complex>,
    pub s22: Option<Complex>,
    pub z_s11: Complex,
}

impl SParameterPoint {
    /// Returns the two-port S-parameter matrix when all four values are present.
    pub fn s_parameter_matrix(&self) -> Option<SParameterMatrix> {
        Some(SParameterMatrix::new(
            self.s11, self.s12?, self.s21?, self.s22?,
        ))
    }

    /// Linearly interpolates all available S-parameters in Cartesian coordinates.
    pub(crate) fn interpolate(
        left: &Self,
        right: &Self,
        frequency_hz: f64,
        ratio: f64,
        reference_impedance_ohm: f64,
    ) -> Self {
        Self {
            frequency_hz,
            s11: interpolate_complex(left.s11, right.s11, ratio),
            s21: interpolate_optional_complex(left.s21, right.s21, ratio),
            s12: interpolate_optional_complex(left.s12, right.s12, ratio),
            s22: interpolate_optional_complex(left.s22, right.s22, ratio),
            z_s11: Complex::ZERO,
        }
        .with_recalculated_impedance(reference_impedance_ohm)
    }

    /// Recalculates the S11 impedance for the supplied port reference.
    pub(crate) fn with_recalculated_impedance(mut self, reference_impedance_ohm: f64) -> Self {
        self.z_s11 = crate::transmission_line_calculator::reflection_to_impedance(
            self.s11,
            reference_impedance_ohm,
        );
        self
    }
}

/// Linearly interpolates a complex value in Cartesian coordinates.
fn interpolate_complex(left: Complex, right: Complex, ratio: f64) -> Complex {
    left + (right - left) * ratio
}

/// Linearly interpolates an optional complex value when both endpoints exist.
fn interpolate_optional_complex(
    left: Option<Complex>,
    right: Option<Complex>,
    ratio: f64,
) -> Option<Complex> {
    left.zip(right)
        .map(|(left, right)| interpolate_complex(left, right, ratio))
}
