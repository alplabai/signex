use serde::{Deserialize, Serialize};

/// Identifies the numeric encoding used by a Touchstone data file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchstoneFormat {
    RealImaginary,
    MagnitudeAngle,
    DecibelAngle,
}
