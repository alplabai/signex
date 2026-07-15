use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoisePoint {
    pub frequency_hz: f64,
    pub fmin_db: f64,
    pub optimum_gamma: Complex,
    pub rn_ohm: f64,
    pub optimum_admittance: Complex,
}
