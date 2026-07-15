use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GainPoint {
    pub frequency_hz: f64,
    pub transducer_gain_linear: f64,
}
