use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformerModel {
    Ideal,
    CoupledInductor,
}

impl fmt::Display for TransformerModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Ideal => "Ideal",
            Self::CoupledInductor => "Coupled inductor",
        };
        formatter.write_str(label)
    }
}
