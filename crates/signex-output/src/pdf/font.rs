//! Font subsetting + embedding for PDFs.
//!
//! v0.8.0 uses PDF standard Type 1 fonts (Helvetica variants). Custom fonts
//! (Roboto, Iosevka subsetting) deferred to v1.0+.
//!
//! Maps `template::FontStyle` to Helvetica variants. Emits Font dict entries
//! for pdf-writer content streams.

use pdf_writer::Ref;

use crate::template::FontStyle;

/// Standard PDF Type 1 fonts available in every PDF reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PdfFont {
    Helvetica,
    HelveticaBold,
    HelveticaOblique,
    HelveticaBoldOblique,
    Courier,
}

impl PdfFont {
    /// Map template FontStyle to a Helvetica variant.
    pub fn for_style(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => PdfFont::Helvetica,
            FontStyle::Bold => PdfFont::HelveticaBold,
            FontStyle::Italic => PdfFont::HelveticaOblique,
            FontStyle::BoldItalic => PdfFont::HelveticaBoldOblique,
        }
    }

    /// PostScript name for this font as used in PDF.
    pub fn ps_name(&self) -> &'static str {
        match self {
            PdfFont::Helvetica => "Helvetica",
            PdfFont::HelveticaBold => "Helvetica-Bold",
            PdfFont::HelveticaOblique => "Helvetica-Oblique",
            PdfFont::HelveticaBoldOblique => "Helvetica-BoldOblique",
            PdfFont::Courier => "Courier",
        }
    }

    /// Character width in 1000ths of font size unit for the string.
    /// Helvetica and Courier are monospace-compatible. For simplicity,
    /// assume fixed-width glyph advancement.
    pub fn glyph_width_approx(&self) -> f32 {
        // Helvetica glyphs vary, but a rough average is 0.55 em per char.
        // For initial implementation, use 0.55.
        0.55
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_style_maps_to_helvetica_variant() {
        assert_eq!(PdfFont::for_style(FontStyle::Normal), PdfFont::Helvetica);
        assert_eq!(PdfFont::for_style(FontStyle::Bold), PdfFont::HelveticaBold);
        assert_eq!(PdfFont::for_style(FontStyle::Italic), PdfFont::HelveticaOblique);
        assert_eq!(
            PdfFont::for_style(FontStyle::BoldItalic),
            PdfFont::HelveticaBoldOblique
        );
    }

    #[test]
    fn postscript_names_correct() {
        assert_eq!(PdfFont::Helvetica.ps_name(), "Helvetica");
        assert_eq!(PdfFont::HelveticaBold.ps_name(), "Helvetica-Bold");
        assert_eq!(PdfFont::HelveticaOblique.ps_name(), "Helvetica-Oblique");
        assert_eq!(
            PdfFont::HelveticaBoldOblique.ps_name(),
            "Helvetica-BoldOblique"
        );
        assert_eq!(PdfFont::Courier.ps_name(), "Courier");
    }

    #[test]
    fn font_catalog_registers_unique() {
        let mut cat = FontCatalog::new();
        let r1 = cat.register(PdfFont::Helvetica);
        let r2 = cat.register(PdfFont::Helvetica);
        assert_eq!(r1, r2);
        let r3 = cat.register(PdfFont::HelveticaBold);
        assert_ne!(r1, r3);
    }

    #[test]
    fn font_catalog_get_retrieves() {
        let mut cat = FontCatalog::new();
        cat.register(PdfFont::Helvetica);
        assert!(cat.get(PdfFont::Helvetica).is_some());
        assert!(cat.get(PdfFont::HelveticaBold).is_none());
    }
}
