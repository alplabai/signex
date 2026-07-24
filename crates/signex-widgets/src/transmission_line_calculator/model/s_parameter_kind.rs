use serde::{Deserialize, Serialize};

/// Identifies whether a data block contains one-port or two-port parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SParameterKind {
    S1P,
    S2P,
}
