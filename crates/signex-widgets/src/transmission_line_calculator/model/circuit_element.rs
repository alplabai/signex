use crate::transmission_line_calculator::{Complex, ElementKind};
use serde::{Deserialize, Serialize};

/// Describes one legacy solver element by name, kind, scalar value, and state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitElement {
    pub name: String,
    pub kind: ElementKind,
    pub value: f64,
    pub enabled: bool,
}

impl CircuitElement {
    /// Creates a load element from the supplied complex impedance.
    pub fn load(impedance: Complex) -> Self {
        Self {
            name: "Load".to_string(),
            kind: ElementKind::Load,
            value: impedance.re,
            enabled: true,
        }
    }

    /// Creates an enabled circuit element with the supplied name, kind, and value.
    pub fn new(name: impl Into<String>, kind: ElementKind, value: f64) -> Self {
        Self {
            name: name.into(),
            kind,
            value,
            enabled: true,
        }
    }
}
