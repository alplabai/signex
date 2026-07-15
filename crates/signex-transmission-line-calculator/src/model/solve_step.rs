use crate::{CircuitElement, Complex};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolveStep {
    pub element: CircuitElement,
    pub impedance: Complex,
    pub normalized_impedance: Complex,
    pub reflection_coefficient: Complex,
}
