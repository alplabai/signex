use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpedanceArc {
    pub variant_index: usize,
    pub element_index: usize,
    pub element_name: String,
    pub points: Vec<Complex>,
}
