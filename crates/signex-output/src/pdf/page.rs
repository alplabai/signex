//! Page-size geometry. The millimetre table lives once, in
//! [`SheetSize`]; a [`PageSize`] is its `Copy` form for print options, so
//! the printed page and the drawn sheet cannot disagree.

use signex_types::schematic::{SheetOrientation, SheetSize};

use super::PageSize;

impl PageSize {
    /// The sheet size this page prints on. `Custom` keeps its dimensions
    /// as given.
    pub fn sheet_size(self) -> SheetSize {
        match self {
            PageSize::IsoA0 => SheetSize::IsoA0,
            PageSize::IsoA1 => SheetSize::IsoA1,
            PageSize::IsoA2 => SheetSize::IsoA2,
            PageSize::IsoA3 => SheetSize::IsoA3,
            PageSize::IsoA4 => SheetSize::IsoA4,
            PageSize::IsoA5 => SheetSize::IsoA5,
            PageSize::IsoB5 => SheetSize::IsoB5,
            PageSize::AnsiA => SheetSize::AnsiA,
            PageSize::AnsiB => SheetSize::AnsiB,
            PageSize::AnsiC => SheetSize::AnsiC,
            PageSize::AnsiD => SheetSize::AnsiD,
            PageSize::AnsiE => SheetSize::AnsiE,
            PageSize::UsLetter => SheetSize::Letter,
            PageSize::UsLegal => SheetSize::Legal,
            PageSize::Tabloid => SheetSize::Tabloid,
            PageSize::Custom {
                width_mm,
                height_mm,
            } => SheetSize::Custom {
                width_mm,
                height_mm,
            },
        }
    }

    /// The PDF page and orientation for a sheet's typed size, so the print
    /// modal opens on the sheet's own paper.
    ///
    /// Every named size maps to its preset with the sheet's orientation.
    /// Custom sheets arrive as `Custom` at their drawn dimensions, with the
    /// orientation their shape describes; an unrecognised size prints as A4
    /// landscape, as the canvas draws it.
    pub fn from_sheet_size(
        size: &SheetSize,
        orientation: SheetOrientation,
    ) -> (Self, SheetOrientation) {
        let preset = match size {
            SheetSize::IsoA0 => PageSize::IsoA0,
            SheetSize::IsoA1 => PageSize::IsoA1,
            SheetSize::IsoA2 => PageSize::IsoA2,
            SheetSize::IsoA3 => PageSize::IsoA3,
            SheetSize::IsoA4 => PageSize::IsoA4,
            SheetSize::IsoA5 => PageSize::IsoA5,
            SheetSize::IsoB5 => PageSize::IsoB5,
            SheetSize::AnsiA => PageSize::AnsiA,
            SheetSize::AnsiB => PageSize::AnsiB,
            SheetSize::AnsiC => PageSize::AnsiC,
            SheetSize::AnsiD => PageSize::AnsiD,
            SheetSize::AnsiE => PageSize::AnsiE,
            SheetSize::Letter => PageSize::UsLetter,
            SheetSize::Legal => PageSize::UsLegal,
            SheetSize::Tabloid => PageSize::Tabloid,
            SheetSize::Unrecognized(_) => {
                return (PageSize::IsoA4, SheetOrientation::Landscape);
            }
            SheetSize::Custom { .. } => {
                let (width_mm, height_mm) = size.dimensions_mm(orientation);
                let shape = if width_mm >= height_mm {
                    SheetOrientation::Landscape
                } else {
                    SheetOrientation::Portrait
                };
                return (
                    PageSize::Custom {
                        width_mm,
                        height_mm,
                    },
                    shape,
                );
            }
        };
        (preset, orientation)
    }

    /// Portrait `(width_mm, height_mm)`. For landscape, swap them.
    pub fn portrait_dimensions_mm(self) -> (f64, f64) {
        self.sheet_size().dimensions_mm(SheetOrientation::Portrait)
    }

    /// Effective `(width_mm, height_mm)` honouring orientation. Custom
    /// sizes are not rotated (user supplied them as-is).
    pub fn dimensions_mm(self, orientation: SheetOrientation) -> (f64, f64) {
        self.sheet_size().dimensions_mm(orientation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page table as it stood before it was merged into `SheetSize`
    /// (#2109): portrait `(width_mm, height_mm)` per size, as literals.
    #[test]
    fn every_page_size_keeps_its_millimetres() {
        let table = [
            (PageSize::IsoA0, (841.0, 1189.0)),
            (PageSize::IsoA1, (594.0, 841.0)),
            (PageSize::IsoA2, (420.0, 594.0)),
            (PageSize::IsoA3, (297.0, 420.0)),
            (PageSize::IsoA4, (210.0, 297.0)),
            (PageSize::IsoA5, (148.0, 210.0)),
            (PageSize::AnsiA, (215.9, 279.4)),
            (PageSize::AnsiB, (279.4, 431.8)),
            (PageSize::AnsiC, (431.8, 558.8)),
            (PageSize::AnsiD, (558.8, 863.6)),
            (PageSize::AnsiE, (863.6, 1117.6)),
            (PageSize::UsLetter, (215.9, 279.4)),
            (PageSize::UsLegal, (215.9, 355.6)),
            // B5 and Tabloid were `Custom` at these dimensions before.
            (PageSize::IsoB5, (182.0, 257.0)),
            (PageSize::Tabloid, (279.4, 431.8)),
        ];
        for (page, (w, h)) in table {
            assert_eq!(page.portrait_dimensions_mm(), (w, h), "{page:?} portrait");
            assert_eq!(
                page.dimensions_mm(SheetOrientation::Portrait),
                (w, h),
                "{page:?}"
            );
            assert_eq!(
                page.dimensions_mm(SheetOrientation::Landscape),
                (h, w),
                "{page:?} landscape"
            );
        }
        let custom = PageSize::Custom {
            width_mm: 123.4,
            height_mm: 56.7,
        };
        assert_eq!(custom.portrait_dimensions_mm(), (123.4, 56.7));
    }

    #[test]
    fn b5_and_tabloid_sheets_map_to_presets() {
        for (size, page) in [
            (SheetSize::IsoB5, PageSize::IsoB5),
            (SheetSize::Tabloid, PageSize::Tabloid),
        ] {
            for o in [SheetOrientation::Landscape, SheetOrientation::Portrait] {
                assert_eq!(PageSize::from_sheet_size(&size, o), (page, o));
                assert_eq!(page.sheet_size(), size);
            }
        }
    }

    #[test]
    fn iso_a4_portrait_is_210_by_297() {
        assert_eq!(
            PageSize::IsoA4.dimensions_mm(SheetOrientation::Portrait),
            (210.0, 297.0),
        );
    }

    #[test]
    fn iso_a4_landscape_swaps() {
        assert_eq!(
            PageSize::IsoA4.dimensions_mm(SheetOrientation::Landscape),
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
        assert_eq!(s.dimensions_mm(SheetOrientation::Portrait), (100.0, 200.0));
        // Landscape request ignored for custom sizes — user supplied
        // exactly what they want.
        assert_eq!(s.dimensions_mm(SheetOrientation::Landscape), (100.0, 200.0));
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
        assert_eq!(orientation, SheetOrientation::Portrait);
        let (size, o) = SheetSize::parse("Foolscap");
        let (page, orientation) = PageSize::from_sheet_size(&size, o);
        assert_eq!(page, PageSize::IsoA4);
        assert_eq!(orientation, SheetOrientation::Landscape);
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
