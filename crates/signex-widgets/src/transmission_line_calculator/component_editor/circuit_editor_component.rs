use crate::transmission_line_calculator::{CustomInterpolation, TransformerModel};

use super::CircuitComponentKind;

/// Stores the editable text fields and options for one circuit component.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitEditorComponent {
    pub kind: CircuitComponentKind,
    pub primary: String,
    pub secondary: String,
    pub tertiary: String,
    pub tolerance: String,
    pub transformer_model: TransformerModel,
    pub interpolation: CustomInterpolation,
}
