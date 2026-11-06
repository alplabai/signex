//! The Text tool's commit as a plain-value method (#1930).

use signex_types::schematic::{HAlign, Point, TextNote, VAlign};

use super::super::super::*;

impl Signex {
    /// Place a text note reading `text` at `at`, already snapped, in the
    /// project's default text size and font (#2026). Whether it was placed.
    pub(crate) fn place_text_at(&mut self, at: Point, text: String) -> bool {
        let text_note = TextNote {
            uuid: uuid::Uuid::now_v7(),
            text,
            position: at,
            rotation: 0.0,
            font_size: self
                .document_state
                .active_schematic_defaults()
                .placed_text_mm(),
            justify_h: HAlign::Left,
            justify_v: VAlign::default(),
            font_family: self.document_state.placed_font_family(),
            locked: false,
        };
        self.apply_engine_command(
            signex_engine::Command::PlaceTextNote { text_note },
            false,
            false,
        )
    }
}
