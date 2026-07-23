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

    /// Creates the identity, or ideal through, two-port matrix.
    pub const fn identity() -> Self {
        Self::new(Complex::ONE, Complex::ZERO, Complex::ZERO, Complex::ONE)
    }

    /// Creates the ABCD matrix for a series impedance.
    pub const fn series_impedance(impedance: Complex) -> Self {
        Self::new(Complex::ONE, impedance, Complex::ZERO, Complex::ONE)
    }

    /// Creates the ABCD matrix for a shunt admittance.
    pub const fn shunt_admittance(admittance: Complex) -> Self {
        Self::new(Complex::ONE, Complex::ZERO, admittance, Complex::ONE)
    }

    /// Creates a lossless transmission-line ABCD matrix.
    pub fn lossless_transmission_line(
        characteristic_impedance: f64,
        electrical_length_radians: f64,
    ) -> Result<Self, TwoPortError> {
        if !characteristic_impedance.is_finite()
            || characteristic_impedance <= 0.0
            || !electrical_length_radians.is_finite()
        {
            return Err(TwoPortError::InvalidReferenceImpedance);
        }

        let cosine = electrical_length_radians.cos();
        let sine = electrical_length_radians.sin();
        Ok(Self::new(
            Complex::new(cosine, 0.0),
            Complex::new(0.0, characteristic_impedance * sine),
            Complex::new(0.0, sine / characteristic_impedance),
            Complex::new(cosine, 0.0),
        ))
    }

    /// Creates the ABCD matrix for an ideal voltage transformer.
    pub fn ideal_transformer(turns_ratio: f64) -> Result<Self, TwoPortError> {
        if !turns_ratio.is_finite() || turns_ratio <= 0.0 {
            return Err(TwoPortError::InvalidReferenceImpedance);
        }
        Ok(Self::new(
            Complex::new(turns_ratio, 0.0),
            Complex::ZERO,
            Complex::ZERO,
            Complex::new(1.0 / turns_ratio, 0.0),
        ))
    }

    /// Returns the determinant of the matrix.
    pub fn determinant(self) -> Complex {
        self.a * self.d - self.b * self.c
    }

    /// Cascades this source-side network with the supplied load-side network.
    pub fn cascade(self, load_side: Self) -> Self {
        Self::new(
            self.a * load_side.a + self.b * load_side.c,
            self.a * load_side.b + self.b * load_side.d,
            self.c * load_side.a + self.d * load_side.c,
            self.c * load_side.b + self.d * load_side.d,
        )
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
