use crate::transmission_line_calculator::{Complex, GainCirclePort};
use serde::{Deserialize, Serialize};

/// Describes a constant-gain circle in the selected source or load plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GainCircle {
    pub frequency_hz: f64,
    pub port: GainCirclePort,
    pub target_gain_db: f64,
    pub center: Complex,
    pub radius: f64,
}
