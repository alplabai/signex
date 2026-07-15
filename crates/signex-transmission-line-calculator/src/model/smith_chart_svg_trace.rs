use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmithChartSvgTrace {
    pub label: String,
    pub color: String,
    pub points: Vec<Complex>,
}
