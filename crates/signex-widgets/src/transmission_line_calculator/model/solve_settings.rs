use crate::transmission_line_calculator::DEFAULT_REFERENCE_IMPEDANCE_OHM;
use serde::{Deserialize, Serialize};

/// Configures frequency, reference impedance, and propagation velocity for solving.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolveSettings {
    pub frequency_hz: f64,
    pub reference_impedance_ohm: f64,
    pub velocity_factor: f64,
}

impl Default for SolveSettings {
    /// Creates the default value for this type.
    fn default() -> Self {
        Self {
            frequency_hz: 1.0e9,
            reference_impedance_ohm: DEFAULT_REFERENCE_IMPEDANCE_OHM,
            velocity_factor: 1.0,
        }
    }
}
