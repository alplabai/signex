use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Stores Touchstone noise parameters at one frequency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoisePoint {
    pub frequency_hz: f64,
    pub fmin_db: f64,
    pub optimum_gamma: Complex,
    pub rn_ohm: f64,
    pub optimum_admittance: Complex,
}
