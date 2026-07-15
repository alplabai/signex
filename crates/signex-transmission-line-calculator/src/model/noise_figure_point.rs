use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseFigurePoint {
    pub frequency_hz: f64,
    pub noise_factor_linear: f64,
}
