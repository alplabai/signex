use crate::transmission_line_calculator::{CircuitElement, Complex};
use serde::{Deserialize, Serialize};

/// Records the network state after applying one circuit element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolveStep {
    pub element: CircuitElement,
    pub impedance: Complex,
    pub normalized_impedance: Complex,
    pub reflection_coefficient: Complex,
}
