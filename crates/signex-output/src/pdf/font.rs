//! Font embedding + subsetting for PDFs.
//!
//! v0.8 references PDF standard-14 Type1 fonts (Helvetica variants + Courier
//! variants) via short aliases `F1`–`F4`. The alias-to-`/BaseFont` mapping is
//! emitted once per PDF; every page points at the same four font objects.
//!
//! The Roboto + Iosevka TTF bytes below are bundled at compile time but NOT
//! yet wired into the PDF pipeline — they're parked for v0.9 when the Type0
//! composite-font dict + FontFile2 stream emission lands. The `#[allow(dead_code)]`
//! annotations that follow are deliberate.

#![allow(dead_code)]

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
    /// Every variant of the enum — handy for allocating refs / emitting
    /// the /Font resources dict without duplicating the match list.
    pub const ALL: [PdfFont; 4] = [
        PdfFont::RobotoRegular,
        PdfFont::RobotoBold,
        PdfFont::IosevkaRegular,
        PdfFont::IosevkaBold,
    ];

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

    /// Short alias used inside content streams (`/F1 9 Tf ...`). Matches the
    /// keys emitted in the page's /Font resources dict.
    pub fn alias(&self) -> &'static str {
        match self {
            PdfFont::RobotoRegular => "F1",
            PdfFont::RobotoBold => "F2",
            PdfFont::IosevkaRegular => "F3",
            PdfFont::IosevkaBold => "F4",
        }
    }

    /// PDF standard-14 Type1 font name we fall back to while full Type0
    /// composite-font emission is deferred to v0.9. Roboto → Helvetica,
    /// Iosevka → Courier (monospace). Every PDF reader ships these by
    /// spec, so text always renders even without a /FontFile2 stream.
    pub fn standard_ps_name(&self) -> &'static str {
        match self {
            PdfFont::RobotoRegular => "Helvetica",
            PdfFont::RobotoBold => "Helvetica-Bold",
            PdfFont::IosevkaRegular => "Courier",
            PdfFont::IosevkaBold => "Courier-Bold",
        }
    }

    /// PostScript base name for this font (used in /BaseFont). Matches the
    /// embedded TTF — used once full Type0 emission lands in v0.9.
    #[allow(dead_code)]
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

/// Resolve alias (`F1`..`F4`) to PdfFont.
pub fn font_for_alias(alias: &str) -> PdfFont {
    match alias {
        "F1" => PdfFont::RobotoRegular,
        "F2" => PdfFont::RobotoBold,
        "F3" => PdfFont::IosevkaRegular,
        "F4" => PdfFont::IosevkaBold,
        _ => PdfFont::RobotoRegular,
    }
}

/// Choose the most suitable alias for the given text.
/// Keeps the preferred alias when glyph coverage is full.
pub fn best_alias_for_text(preferred_alias: &str, text: &str) -> &'static str {
    let preferred = font_for_alias(preferred_alias);
    if glyph_coverage(preferred, text) >= 1.0 {
        return preferred.alias();
    }

    let mut best = preferred;
    let mut best_cov = glyph_coverage(preferred, text);
    for candidate in PdfFont::ALL {
        let cov = glyph_coverage(candidate, text);
        if cov > best_cov {
            best_cov = cov;
            best = candidate;
        }
    }

    best.alias()
}

/// Convert text to a PDF-safe Latin-1-ish representation so standard-14
/// font fallback does not drop characters.
pub fn sanitize_pdf_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let mapped = match ch {
            'Ğ' => 'G',
            'ğ' => 'g',
            'İ' => 'I',
            'ı' => 'i',
            'Ş' => 'S',
            'ş' => 's',
            'Ç' => 'C',
            'ç' => 'c',
            'Ö' => 'O',
            'ö' => 'o',
            'Ü' => 'U',
            'ü' => 'u',
            _ => ch,
        };

        let code = mapped as u32;
        if (0x20..=0x7E).contains(&code) || (0xA0..=0xFF).contains(&code) {
            out.push(mapped);
        } else if mapped == '\n' || mapped == '\r' || mapped == '\t' {
            out.push(' ');
        } else {
            out.push('?');
        }
    }
    out
}

/// Approximate text advance using embedded TTF metrics at the given size.
pub fn text_advance_pt(alias: &str, text: &str, size_pt: f32) -> f32 {
    let font = font_for_alias(alias);
    let Some(face) = font.face() else {
        return size_pt.max(1.0) * text.chars().count() as f32 * 0.5;
    };

    let upm = face.units_per_em() as f32;
    if upm <= 0.0 {
        return size_pt.max(1.0) * text.chars().count() as f32 * 0.5;
    }

    let scale = size_pt.max(1.0) / upm;
    let mut advance = 0.0_f32;
    for ch in text.chars() {
        if let Some(gid) = face.glyph_index(ch) {
            advance += face
                .glyph_hor_advance(gid)
                .map(|v| v as f32 * scale)
                .unwrap_or(upm * scale * 0.5);
        } else {
            advance += upm * scale * 0.5;
        }
    }
    advance
}

fn glyph_coverage(font: PdfFont, text: &str) -> f32 {
    let Some(face) = font.face() else {
        return 0.0;
    };

    let mut total = 0usize;
    let mut ok = 0usize;
    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }
        total += 1;
        if face.glyph_index(ch).is_some() {
            ok += 1;
        }
    }

    if total == 0 {
        1.0
    } else {
        ok as f32 / total as f32
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
    fn aliases_and_standard_fallbacks() {
        assert_eq!(PdfFont::RobotoRegular.alias(), "F1");
        assert_eq!(PdfFont::IosevkaBold.alias(), "F4");
        assert_eq!(PdfFont::RobotoRegular.standard_ps_name(), "Helvetica");
        assert_eq!(PdfFont::RobotoBold.standard_ps_name(), "Helvetica-Bold");
        assert_eq!(PdfFont::IosevkaRegular.standard_ps_name(), "Courier");
        assert_eq!(PdfFont::IosevkaBold.standard_ps_name(), "Courier-Bold");
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
