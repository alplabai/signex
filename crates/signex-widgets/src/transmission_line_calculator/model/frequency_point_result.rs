use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Stores solved impedance and reflection coefficient at one frequency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrequencyPointResult {
    pub frequency_hz: f64,
    pub impedance: Complex,
    pub reflection_coefficient: Complex,
}
