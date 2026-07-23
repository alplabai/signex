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
}
