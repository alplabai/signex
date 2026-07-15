use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoiseFigureCircle {
    pub frequency_hz: f64,
    pub target_noise_figure_db: f64,
    pub center: Complex,
    pub radius: f64,
}
