//! Page-size geometry. ISO 216 (A0-A5) + ANSI/ASME Y14.1 (A-E) + US
//! Letter + US Legal, with portrait orientation giving the "tall"
//! dimension. Values in millimetres.

use signex_types::schematic::{SheetOrientation, SheetSize};

use super::{Orientation, PageSize};

impl PageSize {
    /// The PDF page and orientation for a sheet's typed size, so the print
    /// modal opens on the sheet's own paper.
    ///
    /// Presets with a matching [`PageSize`] map to it. B5, Tabloid and custom
    /// sheets arrive as `Custom` at their drawn dimensions, with the
    /// orientation their shape describes; an unrecognised size prints as A4
    /// landscape, as the canvas draws it.
    pub fn from_sheet_size(size: &SheetSize, orientation: SheetOrientation) -> (Self, Orientation) {
        let preset = match size {
            SheetSize::IsoA0 => Some(PageSize::IsoA0),
            SheetSize::IsoA1 => Some(PageSize::IsoA1),
            SheetSize::IsoA2 => Some(PageSize::IsoA2),
            SheetSize::IsoA3 => Some(PageSize::IsoA3),
            SheetSize::IsoA4 => Some(PageSize::IsoA4),
            SheetSize::IsoA5 => Some(PageSize::IsoA5),
            SheetSize::AnsiA => Some(PageSize::AnsiA),
            SheetSize::AnsiB => Some(PageSize::AnsiB),
            SheetSize::AnsiC => Some(PageSize::AnsiC),
            SheetSize::AnsiD => Some(PageSize::AnsiD),
            SheetSize::AnsiE => Some(PageSize::AnsiE),
            SheetSize::Letter => Some(PageSize::UsLetter),
            SheetSize::Legal => Some(PageSize::UsLegal),
            SheetSize::IsoB5
            | SheetSize::Tabloid
            | SheetSize::Custom { .. }
            | SheetSize::Unrecognized(_) => None,
        };
        if let Some(page) = preset {
            let orientation = match orientation {
                SheetOrientation::Landscape => Orientation::Landscape,
                SheetOrientation::Portrait => Orientation::Portrait,
            };
            return (page, orientation);
        }
        if !size.is_recognized() {
            return (PageSize::IsoA4, Orientation::Landscape);
        }
        let (width_mm, height_mm) = size.dimensions_mm(orientation);
        let shape = if width_mm >= height_mm {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        };
        (
            PageSize::Custom {
                width_mm,
                height_mm,
            },
            shape,
        )
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
            PageSize::Custom {
                width_mm,
                height_mm,
            } => (width_mm, height_mm),
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
    fn pdf_page_and_sheet_size_agree_on_every_pickable_size() {
        for size in SheetSize::PICKABLE {
            for sheet_orientation in [SheetOrientation::Landscape, SheetOrientation::Portrait] {
                let (page, orientation) = PageSize::from_sheet_size(&size, sheet_orientation);
                let (pw, ph) = page.dimensions_mm(orientation);
                let (sw, sh) = size.dimensions_mm(sheet_orientation);
                assert!(
                    (pw - sw).abs() < 1e-9 && (ph - sh).abs() < 1e-9,
                    "{size:?} {sheet_orientation:?}: page {pw}x{ph} vs sheet {sw}x{sh}"
                );
            }
        }
    }

    #[test]
    fn custom_and_unrecognized_sheets_map_to_a_page() {
        let (size, o) = SheetSize::parse("Custom 100x200");
        let (page, orientation) = PageSize::from_sheet_size(&size, o);
        assert_eq!(page.dimensions_mm(orientation), (100.0, 200.0));
        assert_eq!(orientation, Orientation::Portrait);
        let (size, o) = SheetSize::parse("Foolscap");
        let (page, orientation) = PageSize::from_sheet_size(&size, o);
        assert_eq!(page, PageSize::IsoA4);
        assert_eq!(orientation, Orientation::Landscape);
    }

    /// #1199 — a sheet stored with a short ANSI or US name prints on that
    /// paper, as the PDF page table did before #1188, not on A4.
    #[test]
    fn short_ansi_and_us_names_print_on_their_paper() {
        for (token, expected) in [
            ("A", PageSize::AnsiA),
            ("E", PageSize::AnsiE),
            ("USLetter", PageSize::UsLetter),
            ("USLegal portrait", PageSize::UsLegal),
        ] {
            let (size, o) = SheetSize::parse(token);
            assert_eq!(PageSize::from_sheet_size(&size, o).0, expected, "{token}");
        }
    }
}
