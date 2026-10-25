//! Designator and value text on the symbol canvas.
//!
//! Drawn left/top anchored like a `Text` graphic, which is also how
//! `state::field_bounds` sizes the hit box, so what the user sees is
//! what a press grabs.

use super::*;
use state::FieldKey;

impl SymbolCanvas<'_> {
    /// The designator and value as renderer text, in the selected colour
    /// when the field (or the whole symbol) is selected.
    pub(super) fn field_texts(
        &self,
        selected: &Option<SymbolSelection>,
    ) -> Vec<signex_renderer::schematic::TextInput> {
        [FieldKey::Reference, FieldKey::Value]
            .into_iter()
            .map(|key| {
                let is_selected = match selected {
                    Some(SymbolSelection::Field(k)) => *k == key,
                    Some(SymbolSelection::All) => true,
                    _ => false,
                };
                let [x, y] = state::field_pos(self.symbol, key, self.active_part);
                signex_renderer::schematic::TextInput {
                    content: state::field_text(self.symbol, key).to_owned(),
                    position: [x as f32, y as f32],
                    size_mm: state::FIELD_TEXT_SIZE_MM as f32,
                    color: to_rgba(if is_selected {
                        self.selected_color
                    } else {
                        self.text_color
                    }),
                    bold: false,
                    italic: false,
                    rotation_rad: 0.0,
                    h_align: HAlign::Left,
                    v_align: VAlign::Top,
                }
            })
            .collect()
    }
}
