use crate::transmission_line_calculator::SParameterMatrix;
use serde::{Deserialize, Serialize};

/// Stores a solved two-port S-parameter matrix at one frequency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TwoPortSParameterPoint {
    pub frequency_hz: f64,
    pub s_parameters: SParameterMatrix,
}
