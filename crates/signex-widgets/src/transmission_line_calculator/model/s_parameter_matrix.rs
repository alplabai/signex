use crate::transmission_line_calculator::{AbcdMatrix, Complex, TwoPortError};
use serde::{Deserialize, Serialize};

/// Stores the four complex scattering parameters of a two-port network.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SParameterMatrix {
    pub s11: Complex,
    pub s12: Complex,
    pub s21: Complex,
    pub s22: Complex,
}

impl SParameterMatrix {
    /// Creates a two-port scattering-parameter matrix.
    pub const fn new(s11: Complex, s12: Complex, s21: Complex, s22: Complex) -> Self {
        Self { s11, s12, s21, s22 }
    }

    /// Converts power-wave S-parameters to an ABCD matrix.
    ///
    /// The equations match skrf.network.s2a and support unequal complex port
    /// references whose real parts are positive.
    pub fn to_abcd(
        self,
        port_1_reference_impedance: Complex,
        port_2_reference_impedance: Complex,
    ) -> Result<AbcdMatrix, TwoPortError> {
        validate_reference_impedances(port_1_reference_impedance, port_2_reference_impedance)?;

        let normalization = (port_1_reference_impedance.re * port_2_reference_impedance.re).sqrt();
        let denominator = self.s21 * (2.0 * normalization);
        if denominator.magnitude() <= f64::EPSILON {
            return Err(TwoPortError::SingularSParameterMatrix);
        }

        let s12_s21 = self.s12 * self.s21;
        let one_minus_s11 = Complex::ONE - self.s11;
        let one_minus_s22 = Complex::ONE - self.s22;
        let z1_wave =
            port_1_reference_impedance.conjugate() + self.s11 * port_1_reference_impedance;
        let z2_wave =
            port_2_reference_impedance.conjugate() + self.s22 * port_2_reference_impedance;

        Ok(AbcdMatrix::new(
            (z1_wave * one_minus_s22 + s12_s21 * port_1_reference_impedance) / denominator,
            (z1_wave * z2_wave - s12_s21 * port_1_reference_impedance * port_2_reference_impedance)
                / denominator,
            (one_minus_s11 * one_minus_s22 - s12_s21) / denominator,
            (one_minus_s11 * z2_wave + s12_s21 * port_2_reference_impedance) / denominator,
        ))
    }

    /// Computes the input reflection coefficient for a terminated output port.
    pub fn input_reflection(self, load_reflection: Complex) -> Option<Complex> {
        let denominator = Complex::ONE - self.s22 * load_reflection;
        (denominator.magnitude() > f64::EPSILON)
            .then_some(self.s11 + self.s12 * self.s21 * load_reflection / denominator)
    }

    /// Computes the exact bilateral transducer power gain.
    pub fn transducer_gain(
        self,
        source_reflection: Complex,
        load_reflection: Complex,
    ) -> Option<f64> {
        let source_delivery = 1.0 - source_reflection.magnitude().powi(2);
        let load_delivery = 1.0 - load_reflection.magnitude().powi(2);
        let denominator = (Complex::ONE - self.s11 * source_reflection)
            * (Complex::ONE - self.s22 * load_reflection)
            - self.s12 * self.s21 * source_reflection * load_reflection;
        let denominator_power = denominator.magnitude().powi(2);
        if denominator_power <= f64::EPSILON {
            return None;
        }

        let gain =
            source_delivery * self.s21.magnitude().powi(2) * load_delivery / denominator_power;
        gain.is_finite().then_some(gain)
    }
}

/// Validates two power-wave port reference impedances.
pub(crate) fn validate_reference_impedances(
    port_1_reference_impedance: Complex,
    port_2_reference_impedance: Complex,
) -> Result<(), TwoPortError> {
    let valid = |value: Complex| value.re.is_finite() && value.im.is_finite() && value.re > 0.0;
    if valid(port_1_reference_impedance) && valid(port_2_reference_impedance) {
        Ok(())
    } else {
        Err(TwoPortError::InvalidReferenceImpedance)
    }
}
