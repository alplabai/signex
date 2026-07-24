use serde::{Deserialize, Serialize};

/// Stores the evaluated real and imaginary values of a custom point token.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct CustomPointTokenValue {
    pub(crate) real: f64,
    pub(crate) imaginary: f64,
}
