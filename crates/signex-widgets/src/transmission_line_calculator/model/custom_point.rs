use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Associates a custom impedance sample with its frequency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomPoint {
    pub frequency_hz: f64,
    pub impedance: Complex,
}
