use serde::{Deserialize, Serialize};
use std::fmt;

/// Selects how custom impedance samples are interpolated between frequencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomInterpolation {
    SampleAndHold,
    Linear,
}

pub(crate) const CUSTOM_INTERPOLATIONS: [CustomInterpolation; 2] = [
    CustomInterpolation::Linear,
    CustomInterpolation::SampleAndHold,
];

impl fmt::Display for CustomInterpolation {
    /// Formats the value for user-facing display.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::SampleAndHold => "Sample and hold",
            Self::Linear => "Linear",
        };
        formatter.write_str(label)
    }
}
