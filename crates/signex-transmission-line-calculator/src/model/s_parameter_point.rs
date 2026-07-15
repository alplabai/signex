use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SParameterPoint {
    pub frequency_hz: f64,
    pub s11: Complex,
    pub s21: Option<Complex>,
    pub s12: Option<Complex>,
    pub s22: Option<Complex>,
    pub z_s11: Complex,
}
