use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct CustomPointUrlValue {
    pub(crate) real: f64,
    pub(crate) imaginary: f64,
}
