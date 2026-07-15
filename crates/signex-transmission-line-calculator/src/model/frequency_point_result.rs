use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrequencyPointResult {
    pub frequency_hz: f64,
    pub impedance: Complex,
    pub reflection_coefficient: Complex,
}
