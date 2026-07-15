use crate::{CustomInterpolation, TransformerModel};

use super::CircuitComponentKind;

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
