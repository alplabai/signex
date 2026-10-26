//! Shaped text width, measured with the same shaper the text pipeline draws
//! with (#510).
//!
//! Selection boxes and label flags are sized in the app, but the text inside
//! them is shaped in whatever family the document names. A proportional face
//! is not `chars x advance`, so the width has to come from the shaper: this
//! runs the pipeline's own `shaped_buffer` / `attrs_for_item` for the
//! family and reads the laid-out width.

use super::text::{attrs_for_item, measure_text_bounds_px, shaped_buffer};
use crate::primitive::text::TextItem;

/// Size the measurement shapes at. Widths scale linearly with size, so this
/// only fixes the unit; large enough that hinting does not round the advance.
const PROBE_PX: f32 = 100.0;

/// A font system holding the faces text is drawn with.
pub struct TextMeasurer {
    font_system: cryoglyph::FontSystem,
}

impl TextMeasurer {
    /// `bundled` are the faces compiled into the app. `with_system_fonts`
    /// adds the machine's installed fonts, as the renderer's font system has;
    /// tests pass `false` to stay hermetic.
    pub fn new(bundled: &[&'static [u8]], with_system_fonts: bool) -> Self {
        let mut db = cryoglyph::fontdb::Database::new();
        if with_system_fonts {
            db.load_system_fonts();
        }
        for font in bundled {
            db.load_font_source(cryoglyph::fontdb::Source::Binary(std::sync::Arc::new(
                *font,
            )));
        }
        Self {
            font_system: cryoglyph::FontSystem::new_with_locale_and_db("en-US".to_string(), db),
        }
    }

    /// Width of `text` in em, single line, shaped in `family`.
    pub fn advance_em(&mut self, text: &str, family: &str) -> f32 {
        let item = TextItem {
            content: text.to_string(),
            family: Some(family.into()),
            ..Default::default()
        };
        let attrs = attrs_for_item(&item, family);
        let metrics = cryoglyph::Metrics::new(PROBE_PX, PROBE_PX * 1.2);
        let buffer = shaped_buffer(
            &mut self.font_system,
            &item,
            metrics,
            &attrs,
            [1.0e7, 1.0e7],
        );
        measure_text_bounds_px(&buffer)[0] / PROBE_PX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurer() -> TextMeasurer {
        TextMeasurer::new(
            &[
                include_bytes!("../../tests/fonts/Iosevka-Regular.ttf"),
                include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf"),
            ],
            false,
        )
    }

    #[test]
    fn a_monospace_face_advances_the_same_for_every_glyph() {
        let mut m = measurer();
        let narrow = m.advance_em("iiiiiiii", "Iosevka");
        let wide = m.advance_em("WWWWWWWW", "Iosevka");
        assert!((narrow - wide).abs() < 1e-3, "{narrow} vs {wide}");
    }

    #[test]
    fn a_proportional_face_does_not() {
        let mut m = measurer();
        let narrow = m.advance_em("iiiiiiii", "Roboto");
        let wide = m.advance_em("WWWWWWWW", "Roboto");
        assert!(wide > narrow * 2.0, "{narrow} vs {wide}");
    }
}
