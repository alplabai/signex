use crate::transmission_line_calculator::Complex;
use serde::{Deserialize, Serialize};

/// Stores the Smith-chart path contributed by one element and tolerance variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpedanceArc {
    pub variant_index: usize,
    pub element_index: usize,
    pub element_name: String,
    pub points: Vec<Complex>,
}
