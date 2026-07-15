use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomInterpolation {
    SampleAndHold,
    Linear,
}

impl fmt::Display for CustomInterpolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::SampleAndHold => "Sample and hold",
            Self::Linear => "Linear",
        };
        formatter.write_str(label)
    }
}
