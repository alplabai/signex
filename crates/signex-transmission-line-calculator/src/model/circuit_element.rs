use crate::{Complex, ElementKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitElement {
    pub name: String,
    pub kind: ElementKind,
    pub value: f64,
    pub enabled: bool,
}

impl CircuitElement {
    pub fn load(impedance: Complex) -> Self {
        Self {
            name: "Load".to_string(),
            kind: ElementKind::Load,
            value: impedance.re,
            enabled: true,
        }
    }

    pub fn new(name: impl Into<String>, kind: ElementKind, value: f64) -> Self {
        Self {
            name: name.into(),
            kind,
            value,
            enabled: true,
        }
    }
}
