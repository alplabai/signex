use serde::{Deserialize, Serialize};

/// Selects the source or load reflection plane for a constant-gain circle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GainCirclePort {
    Input,
    Output,
}
