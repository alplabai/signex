//! Page-size geometry. ISO 216 (A0-A5) + ANSI/ASME Y14.1 (A-E) + US
//! Letter + US Legal, with portrait orientation giving the "tall"
//! dimension. Values in millimetres.

use super::{Orientation, PageSize};

impl PageSize {
    /// Parse a KiCad `(paper "...")` string into a `PageSize`.
    ///
    /// KiCad uses strings like `"A4"`, `"A3"`, `"A"`, `"B"`, `"USLetter"`,
    /// `"USLegal"`. Unknown strings fall back to `IsoA4`.
    pub fn from_kicad_str(s: &str) -> Self {
        match s.trim() {
            "A0" => PageSize::IsoA0,
            "A1" => PageSize::IsoA1,
            "A2" => PageSize::IsoA2,
            "A3" => PageSize::IsoA3,
            "A4" => PageSize::IsoA4,
            "A5" => PageSize::IsoA5,
            "A" => PageSize::AnsiA,
            "B" => PageSize::AnsiB,
            "C" => PageSize::AnsiC,
            "D" => PageSize::AnsiD,
            "E" => PageSize::AnsiE,
            "USLetter" => PageSize::UsLetter,
            "USLegal" => PageSize::UsLegal,
            _ => PageSize::IsoA4,
        }
    }

    /// Derive orientation from a KiCad paper-size string.
    ///
    /// KiCad schematics default to landscape for A-series except A4 which is
    /// portrait, and landscape for all ANSI sizes. The `portrait` flag in the
    /// KiCad `(paper ...)` node overrides this; pass it when present.
    pub fn default_orientation_for_kicad(s: &str) -> Orientation {
        match s.trim() {
            // A4 is conventionally portrait in KiCad schematics.
            "A4" | "A5" => Orientation::Portrait,
            // Everything else defaults to landscape.
            _ => Orientation::Landscape,
        }
    }

    /// Portrait `(width_mm, height_mm)`. For landscape, swap them.
    pub fn portrait_dimensions_mm(self) -> (f64, f64) {
        match self {
            PageSize::IsoA0 => (841.0, 1189.0),
            PageSize::IsoA1 => (594.0, 841.0),
            PageSize::IsoA2 => (420.0, 594.0),
            PageSize::IsoA3 => (297.0, 420.0),
            PageSize::IsoA4 => (210.0, 297.0),
            PageSize::IsoA5 => (148.0, 210.0),
            PageSize::AnsiA => (215.9, 279.4),
            PageSize::AnsiB => (279.4, 431.8),
            PageSize::AnsiC => (431.8, 558.8),
            PageSize::AnsiD => (558.8, 863.6),
            PageSize::AnsiE => (863.6, 1117.6),
            PageSize::UsLetter => (215.9, 279.4),
            PageSize::UsLegal => (215.9, 355.6),
            PageSize::Custom { width_mm, height_mm } => (width_mm, height_mm),
        }
    }

    /// Effective `(width_mm, height_mm)` honouring orientation. Custom
    /// sizes are not rotated (user supplied them as-is).
    pub fn dimensions_mm(self, orientation: Orientation) -> (f64, f64) {
        let (w, h) = self.portrait_dimensions_mm();
        match (self, orientation) {
            (PageSize::Custom { .. }, _) => (w, h),
            (_, Orientation::Portrait) => (w, h),
            (_, Orientation::Landscape) => (h, w),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_a4_portrait_is_210_by_297() {
        assert_eq!(
            PageSize::IsoA4.dimensions_mm(Orientation::Portrait),
            (210.0, 297.0),
        );
    }

    #[test]
    fn iso_a4_landscape_swaps() {
        assert_eq!(
            PageSize::IsoA4.dimensions_mm(Orientation::Landscape),
            (297.0, 210.0),
        );
    }

    #[test]
    fn ansi_a_equals_us_letter() {
        let a = PageSize::AnsiA.portrait_dimensions_mm();
        let b = PageSize::UsLetter.portrait_dimensions_mm();
        assert_eq!(a, b);
    }

    #[test]
    fn custom_not_rotated() {
        let s = PageSize::Custom {
            width_mm: 100.0,
            height_mm: 200.0,
        };
        assert_eq!(s.dimensions_mm(Orientation::Portrait), (100.0, 200.0));
        // Landscape request ignored for custom sizes — user supplied
        // exactly what they want.
        assert_eq!(s.dimensions_mm(Orientation::Landscape), (100.0, 200.0));
    }

    #[test]
    fn a4_cuts_a3_in_half() {
        // ISO 216: each step is the previous cut in half along the long
        // axis. A3 portrait = 297 × 420; A4 portrait = 210 × 297. So A3's
        // long axis (420) = 2 × A4's short axis (210).
        let (a4_w, _) = PageSize::IsoA4.portrait_dimensions_mm();
        let (_, a3_h) = PageSize::IsoA3.portrait_dimensions_mm();
        assert_eq!(a3_h, 2.0 * a4_w);
    }

    #[test]
    fn kicad_paper_strings_round_trip() {
        assert!(matches!(PageSize::from_kicad_str("A4"), PageSize::IsoA4));
        assert!(matches!(PageSize::from_kicad_str("A3"), PageSize::IsoA3));
        assert!(matches!(PageSize::from_kicad_str("A"), PageSize::AnsiA));
        assert!(matches!(PageSize::from_kicad_str("USLetter"), PageSize::UsLetter));
        // Unknown string falls back to A4.
        assert!(matches!(PageSize::from_kicad_str("unknown"), PageSize::IsoA4));
    }

    #[test]
    fn kicad_orientation_defaults() {
        assert!(matches!(
            PageSize::default_orientation_for_kicad("A4"),
            Orientation::Portrait
        ));
        assert!(matches!(
            PageSize::default_orientation_for_kicad("A3"),
            Orientation::Landscape
        ));
        assert!(matches!(
            PageSize::default_orientation_for_kicad("A"),
            Orientation::Landscape
        ));
    }
}
