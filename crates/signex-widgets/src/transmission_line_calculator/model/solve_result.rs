use crate::transmission_line_calculator::{Complex, SolveStep};
use serde::{Deserialize, Serialize};

/// Contains the final network solution and every intermediate element step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolveResult {
    pub impedance: Complex,
    pub normalized_impedance: Complex,
    pub reflection_coefficient: Complex,
    pub admittance: Complex,
    pub normalized_admittance: Complex,
    pub return_loss_db: f64,
    pub vswr: f64,
    pub chart_x: f64,
    pub chart_y: f64,
    pub steps: Vec<SolveStep>,
}
