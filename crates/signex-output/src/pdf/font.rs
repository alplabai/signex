//! Bundled Roboto + Iosevka faces for PDF export: style mapping, the
//! `/F1`-`/F4` content-stream aliases, and TTF-metric text measurement.
//!
//! The faces are embedded in the PDF as Type0 fonts by `embed.rs`.

use ttf_parser::Face;

use crate::template::FontStyle;

// Embed font bytes at compile time
const ROBOTO_REGULAR: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf");
const ROBOTO_BOLD: &[u8] = include_bytes!("../../../signex-app/assets/fonts/Roboto-Bold.ttf");
const IOSEVKA_REGULAR: &[u8] =
    include_bytes!("../../../signex-app/assets/fonts/Iosevka-Regular.ttf");
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
            FontStyle::Italic => PdfFont::IosevkaRegular, // Iosevka for italic text
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

    /// PostScript base name for this font (used in /BaseFont). Matches the
    /// embedded TTF.
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

/// Replace control whitespace (newline, CR, tab) with a space. Everything else
/// is kept: the embedded Type0 fonts carry real Unicode glyphs.
pub fn normalize_pdf_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if matches!(ch, '\n' | '\r' | '\t') {
                ' '
            } else {
                ch
            }
        })
        .collect()
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
        // A character the face lacks is drawn as `.notdef` (glyph 0), so
        // measure that glyph's real advance, matching the PDF's /W array.
        let gid = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
        advance += face
            .glyph_hor_advance(gid)
            .map(|v| v as f32 * scale)
            .unwrap_or(upm * scale * 0.5);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_style_maps_to_embedded_variants() {
        assert_eq!(
            PdfFont::for_style(FontStyle::Normal),
            PdfFont::RobotoRegular
        );
        assert_eq!(PdfFont::for_style(FontStyle::Bold), PdfFont::RobotoBold);
        assert_eq!(
            PdfFont::for_style(FontStyle::Italic),
            PdfFont::IosevkaRegular
        );
        assert_eq!(
            PdfFont::for_style(FontStyle::BoldItalic),
            PdfFont::IosevkaBold
        );
    }

    #[test]
    fn base_names_correct() {
        assert_eq!(PdfFont::RobotoRegular.base_name(), "Roboto");
        assert_eq!(PdfFont::RobotoBold.base_name(), "Roboto-Bold");
        assert_eq!(PdfFont::IosevkaRegular.base_name(), "Iosevka");
        assert_eq!(PdfFont::IosevkaBold.base_name(), "Iosevka-Bold");
    }

    #[test]
    fn aliases_round_trip() {
        for font in PdfFont::ALL {
            assert_eq!(font_for_alias(font.alias()), font);
        }
        assert_eq!(PdfFont::RobotoRegular.alias(), "F1");
        assert_eq!(PdfFont::IosevkaBold.alias(), "F4");
    }

    #[test]
    fn normalize_keeps_unicode_and_flattens_whitespace() {
        assert_eq!(normalize_pdf_text("10µF\n±5%\tÖ"), "10µF ±5% Ö");
    }

    #[test]
    fn font_bytes_embedded() {
        assert!(!ROBOTO_REGULAR.is_empty());
        assert!(!ROBOTO_BOLD.is_empty());
        assert!(!IOSEVKA_REGULAR.is_empty());
        assert!(!IOSEVKA_BOLD.is_empty());
    }

    #[test]
    fn fonts_parse_successfully() {
        assert!(PdfFont::RobotoRegular.face().is_some());
        assert!(PdfFont::RobotoBold.face().is_some());
        assert!(PdfFont::IosevkaRegular.face().is_some());
        assert!(PdfFont::IosevkaBold.face().is_some());
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
            let face = font
                .face()
                .unwrap_or_else(|| panic!("Font {:?} should parse", font));
            assert!(face.units_per_em() > 0, "Font {:?} should have UPM", font);
            assert!(
                face.ascender() > face.descender(),
                "Font {:?} ascender should be > descender",
                font
            );
        }
    }
}
