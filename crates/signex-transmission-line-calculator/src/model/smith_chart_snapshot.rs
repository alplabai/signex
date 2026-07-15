use crate::{SmithChartElement, SmithChartOverlays, SmithChartSettings};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmithChartSnapshot {
    pub circuit: Vec<SmithChartElement>,
    pub settings: SmithChartSettings,
    pub overlays: SmithChartOverlays,
}
