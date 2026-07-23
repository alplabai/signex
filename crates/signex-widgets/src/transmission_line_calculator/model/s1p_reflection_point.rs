use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Stores a one-port reflection sample in rectangular and polar form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct S1pReflectionPoint {
    pub frequency_hz: f64,
    pub reflection_coefficient: Complex,
    pub magnitude: f64,
    pub angle_degrees: f64,
}
