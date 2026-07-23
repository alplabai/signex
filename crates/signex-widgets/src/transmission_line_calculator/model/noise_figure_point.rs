use serde::{Deserialize, Serialize};

/// Stores the linear noise factor calculated at one frequency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseFigurePoint {
    pub frequency_hz: f64,
    pub noise_factor_linear: f64,
}
