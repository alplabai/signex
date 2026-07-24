use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Describes source- and load-plane stability circles at one frequency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StabilityCircle {
    pub frequency_hz: f64,
    pub source_center: Complex,
    pub source_radius: f64,
    pub load_center: Complex,
    pub load_radius: f64,
}
