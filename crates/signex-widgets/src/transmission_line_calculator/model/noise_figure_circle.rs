use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Describes a constant-noise-figure circle in the source reflection plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoiseFigureCircle {
    pub frequency_hz: f64,
    pub target_noise_figure_db: f64,
    pub center: Complex,
    pub radius: f64,
}
