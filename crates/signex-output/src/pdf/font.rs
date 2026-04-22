//! Font embedding + subsetting for PDFs.
//!
//! v0.8.0 embeds Roboto (title blocks) and Iosevka (canvas text) as PDF Type0
//! composite CIDFont entries. Full font bytes are embedded (subsetting deferred
//! to v0.9 — contributes ~30-40 KB per PDF but is much simpler to maintain).
//!
//! Maps `template::FontStyle` to embedded variants. FontCatalog tracks refs
//! for each registered font and can emit their Font dictionaries at PDF write time.

use pdf_writer::Ref;
use ttf_parser::Face;

use crate::template::FontStyle;

// Embed font bytes at compile time
const ROBOTO_REGULAR: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf");
const ROBOTO_BOLD: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Roboto-Bold.ttf");
const IOSEVKA_REGULAR: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Iosevka-Regular.ttf");
const IOSEVKA_BOLD: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Iosevka-Bold.ttf");

/// Embedded font variants, backed by TTF bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PdfFont {
    RobotoRegular,
    RobotoBold,
    IosevkaRegular,
    IosevkaBold,
}

impl PdfFont {
    /// Map template FontStyle to the appropriate font.
    /// Normal → Roboto, Bold → Roboto Bold, Italic/BoldItalic → Iosevka.
    pub fn for_style(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => PdfFont::RobotoRegular,
            FontStyle::Bold => PdfFont::RobotoBold,
            FontStyle::Italic => PdfFont::IosevkaRegular,  // Iosevka for italic text
            FontStyle::BoldItalic => PdfFont::IosevkaBold,
        }
    }

    /// PostScript base name for this font (used in /BaseFont).
    pub fn base_name(&self) -> &'static str {
        match self {
            PdfFont::RobotoRegular => "Roboto",
            PdfFont::RobotoBold => "Roboto-Bold",
            PdfFont::IosevkaRegular => "Iosevka",
            PdfFont::IosevkaBold => "Iosevka-Bold",
        }
    }

    /// Retrieve the embedded TTF bytes for this font.
    pub fn font_bytes(&self) -> &'static [u8] {
        match self {
            PdfFont::RobotoRegular => ROBOTO_REGULAR,
            PdfFont::RobotoBold => ROBOTO_BOLD,
            PdfFont::IosevkaRegular => IOSEVKA_REGULAR,
            PdfFont::IosevkaBold => IOSEVKA_BOLD,
        }
    }

    /// Parse the TTF face for metadata (ascent, descent, bbox, etc.).
    pub fn face(&self) -> Option<Face<'static>> {
        Face::parse(self.font_bytes(), 0).ok()
    }
}

/// A catalog of fonts used in the PDF, mapped to their pdf-writer Refs.
#[derive(Debug, Default)]
pub struct FontCatalog {
    fonts: Vec<(PdfFont, Ref)>,
}

impl FontCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a font, returning its Ref. Same font registered twice returns
    /// the same Ref.
    pub fn register(&mut self, font: PdfFont) -> Ref {
        if let Some((_, ref_id)) = self.fonts.iter().find(|(f, _)| f == &font) {
            return *ref_id;
        }
        let ref_id = Ref::new(100 + self.fonts.len() as i32);
        self.fonts.push((font, ref_id));
        ref_id
    }

    /// Get the Ref for a registered font, or None.
    pub fn get(&self, font: PdfFont) -> Option<Ref> {
        self.fonts.iter().find(|(f, _)| f == &font).map(|(_, r)| *r)
    }

    /// Iterate over all registered fonts.
    pub fn iter(&self) -> impl Iterator<Item = (PdfFont, Ref)> + '_ {
        self.fonts.iter().copied()
    }

    /// Get all embedded font bytes for later embedding. Maps font to its TTF bytes.
    pub fn font_data(&self) -> Vec<(PdfFont, &'static [u8])> {
        self.fonts
            .iter()
            .map(|(font, _)| (*font, font.font_bytes()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_style_maps_to_embedded_variants() {
        assert_eq!(PdfFont::for_style(FontStyle::Normal), PdfFont::RobotoRegular);
        assert_eq!(PdfFont::for_style(FontStyle::Bold), PdfFont::RobotoBold);
        assert_eq!(PdfFont::for_style(FontStyle::Italic), PdfFont::IosevkaRegular);
        assert_eq!(PdfFont::for_style(FontStyle::BoldItalic), PdfFont::IosevkaBold);
    }

    #[test]
    fn base_names_correct() {
        assert_eq!(PdfFont::RobotoRegular.base_name(), "Roboto");
        assert_eq!(PdfFont::RobotoBold.base_name(), "Roboto-Bold");
        assert_eq!(PdfFont::IosevkaRegular.base_name(), "Iosevka");
        assert_eq!(PdfFont::IosevkaBold.base_name(), "Iosevka-Bold");
    }

    #[test]
    fn font_bytes_embedded() {
        assert!(!ROBOTO_REGULAR.is_empty());
        assert!(!ROBOTO_BOLD.is_empty());
        assert!(!IOSEVKA_REGULAR.is_empty());
        assert!(!IOSEVKA_BOLD.is_empty());
    }

    #[test]
    fn font_catalog_registers_unique() {
        let mut cat = FontCatalog::new();
        let r1 = cat.register(PdfFont::RobotoRegular);
        let r2 = cat.register(PdfFont::RobotoRegular);
        assert_eq!(r1, r2);
        let r3 = cat.register(PdfFont::RobotoBold);
        assert_ne!(r1, r3);
    }

    #[test]
    fn font_catalog_get_retrieves() {
        let mut cat = FontCatalog::new();
        cat.register(PdfFont::RobotoRegular);
        assert!(cat.get(PdfFont::RobotoRegular).is_some());
        assert!(cat.get(PdfFont::RobotoBold).is_none());
    }

    #[test]
    fn fonts_parse_successfully() {
        assert!(PdfFont::RobotoRegular.face().is_some());
        assert!(PdfFont::RobotoBold.face().is_some());
        assert!(PdfFont::IosevkaRegular.face().is_some());
        assert!(PdfFont::IosevkaBold.face().is_some());
    }

    #[test]
    fn embeds_roboto_font() {
        // Test that the font catalog can register and reference a font
        let mut cat = FontCatalog::new();
        let roboto_ref = cat.register(PdfFont::RobotoRegular);

        // Verify the font is registered
        assert!(cat.get(PdfFont::RobotoRegular).is_some());
        assert_eq!(cat.get(PdfFont::RobotoRegular), Some(roboto_ref));

        // Verify font metadata is accessible
        let face = PdfFont::RobotoRegular.face().expect("Roboto should parse");
        assert!(face.ascender() != 0, "Font should have ascender");
    }

    #[test]
    fn font_bytes_are_valid_ttf() {
        // Verify all embedded fonts parse as valid TTF
        assert!(PdfFont::RobotoRegular.face().is_some());
        assert!(PdfFont::RobotoBold.face().is_some());
        assert!(PdfFont::IosevkaRegular.face().is_some());
        assert!(PdfFont::IosevkaBold.face().is_some());

        // Verify font data method returns all registered fonts
        let mut cat = FontCatalog::new();
        cat.register(PdfFont::RobotoRegular);
        cat.register(PdfFont::IosevkaRegular);

        let font_data = cat.font_data();
        assert_eq!(font_data.len(), 2);
        assert!(font_data[0].1.len() > 0);
        assert!(font_data[1].1.len() > 0);
    }

    #[test]
    fn all_fonts_have_metrics() {
        // Verify each font can be parsed and has valid metrics
        for font in &[
            PdfFont::RobotoRegular,
            PdfFont::RobotoBold,
            PdfFont::IosevkaRegular,
            PdfFont::IosevkaBold,
        ] {
            let face = font.face().expect(&format!("Font {:?} should parse", font));
            assert!(face.units_per_em() > 0, "Font {:?} should have UPM", font);
            assert!(
                face.ascender() > face.descender(),
                "Font {:?} ascender should be > descender",
                font
            );
        }
    }
}
