use crate::transmission_line_calculator::{
    Complex, SParameterMatrix, TwoPortError,
    model::s_parameter_matrix::validate_reference_impedances,
};
use serde::{Deserialize, Serialize};

/// Stores the transmission, or ABCD, matrix of a two-port network.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AbcdMatrix {
    pub a: Complex,
    pub b: Complex,
    pub c: Complex,
    pub d: Complex,
}

impl AbcdMatrix {
    /// Creates an ABCD matrix from its four complex coefficients.
    pub const fn new(a: Complex, b: Complex, c: Complex, d: Complex) -> Self {
        Self { a, b, c, d }
    }

    /// Returns the determinant of the matrix.
    pub fn determinant(self) -> Complex {
        self.a * self.d - self.b * self.c
    }

    /// Converts the ABCD matrix to power-wave S-parameters.
    ///
    /// The equations match skrf.network.a2s and support unequal complex port
    /// references whose real parts are positive.
    pub fn to_s_parameters(
        self,
        port_1_reference_impedance: Complex,
        port_2_reference_impedance: Complex,
    ) -> Result<SParameterMatrix, TwoPortError> {
        validate_reference_impedances(port_1_reference_impedance, port_2_reference_impedance)?;

        let denominator = self.a * port_2_reference_impedance
            + self.b
            + self.c * port_1_reference_impedance * port_2_reference_impedance
            + self.d * port_1_reference_impedance;
        if denominator.magnitude() <= f64::EPSILON {
            return Err(TwoPortError::SingularAbcdMatrix);
        }

        let normalization = (port_1_reference_impedance.re * port_2_reference_impedance.re).sqrt();
        let s11 = (self.a * port_2_reference_impedance + self.b
            - self.c * port_1_reference_impedance.conjugate() * port_2_reference_impedance
            - self.d * port_1_reference_impedance.conjugate())
            / denominator;
        let s21 = Complex::new(2.0 * normalization, 0.0) / denominator;
        let s12 = self.determinant() * (2.0 * normalization) / denominator;
        let s22 = (Complex::ZERO - self.a * port_2_reference_impedance.conjugate() + self.b
            - self.c * port_1_reference_impedance * port_2_reference_impedance.conjugate()
            + self.d * port_1_reference_impedance)
            / denominator;

        Ok(SParameterMatrix::new(s11, s12, s21, s22))
    }

    /// Transforms a load impedance to the input impedance of the two-port network.
    pub fn input_impedance(self, load_impedance: Complex) -> Result<Complex, TwoPortError> {
        let denominator = self.c * load_impedance + self.d;
        if denominator.magnitude() <= f64::EPSILON {
            return Err(TwoPortError::SingularTermination);
        }
        Ok((self.a * load_impedance + self.b) / denominator)
    }
}
