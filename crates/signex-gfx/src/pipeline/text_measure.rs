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

    /// Cap height over em of the installed `family` (#1797): the face's
    /// OS/2 `sCapHeight`, else the height of its `H` outline. `None` when no
    /// installed face carries that name or neither reading works.
    pub fn cap_height_per_em(&self, family: &str) -> Option<f32> {
        let db = self.font_system.db();
        let query = cryoglyph::fontdb::Query {
            families: &[cryoglyph::fontdb::Family::Name(family)],
            ..Default::default()
        };
        let id = db.query(&query)?;
        db.with_face_data(id, |data, index| {
            ttf_parser::Face::parse(data, index)
                .ok()
                .and_then(|face| cap_height_ratio(&face))
        })?
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
            [NO_WRAP_PX, NO_WRAP_PX],
        );
        measure_text_bounds_px(&buffer)[0] / PROBE_PX
    }
}

/// One drawing command of a glyph outline, in pixels, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutlineCmd {
    Move([f32; 2]),
    Line([f32; 2]),
    Quad([f32; 2], [f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Close,
}

/// The outlines of one shaped text: every glyph's contours positioned in the
/// text box (origin at its top-left), and the box size.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextOutlines {
    pub size_px: [f32; 2],
    pub cmds: Vec<OutlineCmd>,
}

/// Collects a `ttf_parser` outline at a glyph's position and scale.
struct GlyphPath<'a> {
    cmds: &'a mut Vec<OutlineCmd>,
    origin: [f32; 2],
    scale: f32,
}

impl GlyphPath<'_> {
    fn at(&self, x: f32, y: f32) -> [f32; 2] {
        [
            self.origin[0] + x * self.scale,
            self.origin[1] - y * self.scale,
        ]
    }
}

impl ttf_parser::OutlineBuilder for GlyphPath<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.cmds.push(OutlineCmd::Move(p));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.cmds.push(OutlineCmd::Line(p));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.at(x1, y1), self.at(x, y));
        self.cmds.push(OutlineCmd::Quad(c, p));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b, p) = (self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        self.cmds.push(OutlineCmd::Cubic(a, b, p));
    }
    fn close(&mut self) {
        self.cmds.push(OutlineCmd::Close);
    }
}

impl TextMeasurer {
    /// `text` shaped in `family` at `px` em with `line_height_em` lines, as
    /// filled glyph outlines. For text too large for a glyph atlas; single
    /// line per source line, no wrapping.
    pub fn outlines(
        &mut self,
        text: &str,
        family: &str,
        px: f32,
        line_height_em: f32,
    ) -> TextOutlines {
        let item = TextItem {
            content: text.to_string(),
            family: Some(family.into()),
            ..Default::default()
        };
        let attrs = attrs_for_item(&item, family);
        let metrics = cryoglyph::Metrics::new(px, px * line_height_em);
        let buffer = shaped_buffer(
            &mut self.font_system,
            &item,
            metrics,
            &attrs,
            [NO_WRAP_PX, NO_WRAP_PX],
        );
        let size_px = measure_text_bounds_px(&buffer);
        let mut cmds = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let origin = [
                    glyph.x + glyph.x_offset * glyph.font_size,
                    run.line_y + glyph.y - glyph.y_offset * glyph.font_size,
                ];
                self.font_system
                    .db()
                    .with_face_data(glyph.font_id, |data, index| {
                        let Ok(face) = ttf_parser::Face::parse(data, index) else {
                            return;
                        };
                        let upem = f32::from(face.units_per_em());
                        if upem <= 0.0 {
                            return;
                        }
                        let mut path = GlyphPath {
                            cmds: &mut cmds,
                            origin,
                            scale: glyph.font_size / upem,
                        };
                        face.outline_glyph(ttf_parser::GlyphId(glyph.glyph_id), &mut path);
                    });
            }
        }
        TextOutlines { size_px, cmds }
    }
}

/// A wrap width no text reaches: sheet text is never wrapped.
pub(crate) const NO_WRAP_PX: f32 = 1.0e7;

/// `sCapHeight / unitsPerEm`, falling back to the bounding box of `H`.
fn cap_height_ratio(face: &ttf_parser::Face<'_>) -> Option<f32> {
    let upm = f32::from(face.units_per_em());
    let cap = face.capital_height().filter(|&cap| cap > 0).or_else(|| {
        let h = face.glyph_index('H')?;
        face.glyph_bounding_box(h)
            .map(|b| b.y_max)
            .filter(|&y| y > 0)
    })?;
    (upm > 0.0).then(|| f32::from(cap) / upm)
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

    /// Oversize text is drawn from outlines: an `H` has contours inside its
    /// text box, scaled with the em.
    #[test]
    fn outlines_fill_the_text_box_and_scale_with_the_em() {
        let mut m = measurer();
        let small = m.outlines("H", "Iosevka", 100.0, 1.2);
        let large = m.outlines("H", "Iosevka", 5000.0, 1.2);
        assert!(!small.cmds.is_empty() && small.cmds.contains(&OutlineCmd::Close));
        assert!((large.size_px[0] / small.size_px[0] - 50.0).abs() < 0.5);
        let max_x = |o: &TextOutlines| {
            o.cmds
                .iter()
                .filter_map(|c| match c {
                    OutlineCmd::Move(p) | OutlineCmd::Line(p) => Some(p[0]),
                    _ => None,
                })
                .fold(0.0_f32, f32::max)
        };
        assert!(max_x(&large) > 0.0 && max_x(&large) <= large.size_px[0] + 1.0);
    }

    /// Sheet text is never wrapped at the viewport: one line stays one line
    /// at any size, which the GPU pipeline relies on (`NO_WRAP_PX`).
    #[test]
    fn long_text_at_a_huge_size_stays_on_one_line() {
        let mut m = measurer();
        let o = m.outlines("A long note that must not wrap", "Iosevka", 2000.0, 1.2);
        assert!((o.size_px[1] - 2000.0 * 1.2).abs() < 1.0, "{:?}", o.size_px);
    }

    #[test]
    fn a_proportional_face_does_not() {
        let mut m = measurer();
        let narrow = m.advance_em("iiiiiiii", "Roboto");
        let wide = m.advance_em("WWWWWWWW", "Roboto");
        assert!(wide > narrow * 2.0, "{narrow} vs {wide}");
    }

    static SYSTEM: std::sync::OnceLock<std::sync::Mutex<TextMeasurer>> = std::sync::OnceLock::new();

    fn system_cap_height(family: &str) -> Option<f32> {
        SYSTEM
            .get_or_init(|| std::sync::Mutex::new(TextMeasurer::new(&[], true)))
            .lock()
            .ok()?
            .cap_height_per_em(family)
    }

    /// #1797 — a system family is sized by its own cap height: a 1.27 mm
    /// label's capitals measure 1.27 mm, as in the bundled faces. Skips when
    /// the machine has none of the common sans faces.
    #[test]
    fn a_system_family_label_measures_its_requested_cap_height() {
        use crate::primitive::text::{TextSizePolicy, text_px_in};
        signex_types::font::set_cap_height_resolver(system_cap_height);
        let Some(family) = ["DejaVu Sans", "Liberation Sans", "Arial", "Helvetica"]
            .into_iter()
            .find(|f| system_cap_height(f).is_some())
        else {
            eprintln!("no common system font installed; skipping");
            return;
        };
        // The reading must be the face's real capital height, not the 0.72
        // fallback: measure the `H` outline independently.
        let mut db = cryoglyph::fontdb::Database::new();
        db.load_system_fonts();
        let id = db
            .query(&cryoglyph::fontdb::Query {
                families: &[cryoglyph::fontdb::Family::Name(family)],
                ..Default::default()
            })
            .expect("face");
        let outline = db
            .with_face_data(id, |data, index| {
                let face = ttf_parser::Face::parse(data, index).ok()?;
                let h = face.glyph_bounding_box(face.glyph_index('H')?)?;
                Some(f32::from(h.y_max) / f32::from(face.units_per_em()))
            })
            .flatten()
            .expect("H outline");

        let policy = TextSizePolicy::new(0.0, f32::INFINITY);
        let em_px = text_px_in(1.27, 10.0, policy, Some(family));
        let cap_px = em_px * outline;
        assert!((cap_px - 12.7).abs() < 0.03, "{family}: {cap_px} px");
    }
}
