use serde::{Deserialize, Serialize};
use std::fmt;

/// Selects the electrical model used for a transformer element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformerModel {
    Ideal,
    CoupledInductor,
}

pub(crate) const TRANSFORMER_MODELS: [TransformerModel; 2] =
    [TransformerModel::CoupledInductor, TransformerModel::Ideal];

impl fmt::Display for TransformerModel {
    /// Formats the value for user-facing display.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Ideal => "Ideal",
            Self::CoupledInductor => "Coupled inductor",
        };
        formatter.write_str(label)
    }
}
