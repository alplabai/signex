use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchstoneFormat {
    RealImaginary,
    MagnitudeAngle,
    DecibelAngle,
}
