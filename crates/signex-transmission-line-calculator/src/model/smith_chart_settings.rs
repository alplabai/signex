use crate::{DEFAULT_REFERENCE_IMPEDANCE_OHM, ScalarUnit};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmithChartSettings {
    pub frequency_hz: f64,
    pub frequency_unit: ScalarUnit,
    pub reference_impedance_ohm: f64,
    pub span_hz: f64,
    pub span_unit: ScalarUnit,
    pub resolution: usize,
    pub show_ideal: bool,
}

impl Default for SmithChartSettings {
    fn default() -> Self {
        Self {
            frequency_hz: 2.44e9,
            frequency_unit: ScalarUnit::MegaHertz,
            reference_impedance_ohm: DEFAULT_REFERENCE_IMPEDANCE_OHM,
            span_hz: 0.0,
            span_unit: ScalarUnit::MegaHertz,
            resolution: 10,
            show_ideal: false,
        }
    }
}
