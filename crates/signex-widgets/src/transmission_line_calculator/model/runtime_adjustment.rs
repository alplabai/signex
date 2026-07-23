use serde::{Deserialize, Serialize};

/// Stores temporary slider adjustments applied to a circuit element at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct RuntimeAdjustment {
    pub value_slider_percent: Option<f64>,
    pub real_slider_percent: Option<f64>,
    pub imaginary_slider_percent: Option<f64>,
}
