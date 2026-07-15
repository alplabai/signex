use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct S1pReflectionPoint {
    pub frequency_hz: f64,
    pub reflection_coefficient: Complex,
    pub magnitude: f64,
    pub angle_degrees: f64,
}
