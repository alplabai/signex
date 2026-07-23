use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Stores a labelled complex trace and its CSS color for SVG rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmithChartSvgTrace {
    pub label: String,
    pub color: String,
    pub points: Vec<Complex>,
}
