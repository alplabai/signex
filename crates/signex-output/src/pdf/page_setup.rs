//! Page-setup value types and validation for the PDF export modal.
//!
//! The modal edits page size, orientation, margins and scale as text. The
//! rules for turning that text into a usable [`PdfOptions`](super::PdfOptions)
//! field live here, in the domain crate, so the UI only forwards strings and
//! shows the [`PageSetupError`] it gets back.

use std::fmt;

use thiserror::Error;

use super::{Margins, Orientation, PageSize, PdfScale};

/// Largest page edge a PDF viewer accepts (14 400 pt).
pub const MAX_PAGE_EDGE_MM: f64 = 5080.0;
/// Smallest Custom page edge. Below this the default margins would consume the
/// whole page, so a Custom page is never allowed to get that small.
pub const MIN_CUSTOM_EDGE_MM: f64 = 10.0;
/// Largest accepted scale percentage.
pub const MAX_SCALE_PERCENT: f64 = 10_000.0;
/// Printable area every axis must keep after margins (matches the layout floor).
pub const MIN_PRINTABLE_MM: f64 = 1.0;

/// Why a page-setup text field was rejected. The previous value is kept.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PageSetupError {
    #[error("enter a number")]
    Empty,
    #[error("'{0}' is not a number")]
    NotANumber(String),
    #[error("must be greater than 0")]
    NotPositive,
    #[error("must be at most {0}")]
    TooLarge(f64),
    #[error("must be at least {0} mm")]
    TooSmall(f64),
    #[error("no room left on the page for this margin")]
    NoRoom,
}

fn parse_finite_positive(text: &str, max: f64) -> Result<f64, PageSetupError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(PageSetupError::Empty);
    }
    let value: f64 = trimmed
        .parse()
        .map_err(|_| PageSetupError::NotANumber(trimmed.to_string()))?;
    if !value.is_finite() {
        return Err(PageSetupError::NotANumber(trimmed.to_string()));
    }
    if value <= 0.0 {
        return Err(PageSetupError::NotPositive);
    }
    if value > max {
        return Err(PageSetupError::TooLarge(max));
    }
    Ok(value)
}

/// Parse a millimetre length (page edge or margin): finite, > 0, within a PDF page.
pub fn parse_mm(text: &str) -> Result<f64, PageSetupError> {
    parse_finite_positive(text, MAX_PAGE_EDGE_MM)
}

/// Parse a Custom page edge in millimetres: like [`parse_mm`] but at least
/// [`MIN_CUSTOM_EDGE_MM`], so the margins can never swallow the page.
pub fn parse_custom_edge_mm(text: &str) -> Result<f64, PageSetupError> {
    let value = parse_mm(text)?;
    if value < MIN_CUSTOM_EDGE_MM {
        return Err(PageSetupError::TooSmall(MIN_CUSTOM_EDGE_MM));
    }
    Ok(value)
}

/// Parse a scale percentage: finite, > 0, at most [`MAX_SCALE_PERCENT`].
pub fn parse_percent(text: &str) -> Result<f64, PageSetupError> {
    parse_finite_positive(text.trim().trim_end_matches('%'), MAX_SCALE_PERCENT)
}

/// Which page-size entry the picker shows. `Custom` carries no dimensions —
/// those live on [`PageSize::Custom`] — so the list has one stable entry for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSizeKind {
    IsoA0,
    IsoA1,
    IsoA2,
    IsoA3,
    IsoA4,
    IsoA5,
    AnsiA,
    AnsiB,
    AnsiC,
    AnsiD,
    AnsiE,
    UsLetter,
    UsLegal,
    Custom,
}

impl PageSizeKind {
    /// Every picker entry, in display order.
    pub const ALL: [PageSizeKind; 14] = [
        PageSizeKind::IsoA0,
        PageSizeKind::IsoA1,
        PageSizeKind::IsoA2,
        PageSizeKind::IsoA3,
        PageSizeKind::IsoA4,
        PageSizeKind::IsoA5,
        PageSizeKind::AnsiA,
        PageSizeKind::AnsiB,
        PageSizeKind::AnsiC,
        PageSizeKind::AnsiD,
        PageSizeKind::AnsiE,
        PageSizeKind::UsLetter,
        PageSizeKind::UsLegal,
        PageSizeKind::Custom,
    ];

    /// The [`PageSize`] this entry selects. `Custom` keeps the dimensions of
    /// `current` when it already is custom, otherwise it starts from `current`'s
    /// effective size so switching to Custom does not jump the page.
    pub fn select(self, current: PageSize, orientation: Orientation) -> PageSize {
        match self {
            PageSizeKind::IsoA0 => PageSize::IsoA0,
            PageSizeKind::IsoA1 => PageSize::IsoA1,
            PageSizeKind::IsoA2 => PageSize::IsoA2,
            PageSizeKind::IsoA3 => PageSize::IsoA3,
            PageSizeKind::IsoA4 => PageSize::IsoA4,
            PageSizeKind::IsoA5 => PageSize::IsoA5,
            PageSizeKind::AnsiA => PageSize::AnsiA,
            PageSizeKind::AnsiB => PageSize::AnsiB,
            PageSizeKind::AnsiC => PageSize::AnsiC,
            PageSizeKind::AnsiD => PageSize::AnsiD,
            PageSizeKind::AnsiE => PageSize::AnsiE,
            PageSizeKind::UsLetter => PageSize::UsLetter,
            PageSizeKind::UsLegal => PageSize::UsLegal,
            PageSizeKind::Custom => match current {
                PageSize::Custom { .. } => current,
                preset => {
                    let (width_mm, height_mm) = preset.dimensions_mm(orientation);
                    PageSize::Custom {
                        width_mm,
                        height_mm,
                    }
                }
            },
        }
    }
}

impl fmt::Display for PageSizeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PageSizeKind::IsoA0 => "ISO A0",
            PageSizeKind::IsoA1 => "ISO A1",
            PageSizeKind::IsoA2 => "ISO A2",
            PageSizeKind::IsoA3 => "ISO A3",
            PageSizeKind::IsoA4 => "ISO A4",
            PageSizeKind::IsoA5 => "ISO A5",
            PageSizeKind::AnsiA => "ANSI A",
            PageSizeKind::AnsiB => "ANSI B",
            PageSizeKind::AnsiC => "ANSI C",
            PageSizeKind::AnsiD => "ANSI D",
            PageSizeKind::AnsiE => "ANSI E",
            PageSizeKind::UsLetter => "US Letter",
            PageSizeKind::UsLegal => "US Legal",
            PageSizeKind::Custom => "Custom",
        })
    }
}

impl PageSize {
    /// The picker entry for this size.
    pub fn kind(self) -> PageSizeKind {
        match self {
            PageSize::IsoA0 => PageSizeKind::IsoA0,
            PageSize::IsoA1 => PageSizeKind::IsoA1,
            PageSize::IsoA2 => PageSizeKind::IsoA2,
            PageSize::IsoA3 => PageSizeKind::IsoA3,
            PageSize::IsoA4 => PageSizeKind::IsoA4,
            PageSize::IsoA5 => PageSizeKind::IsoA5,
            PageSize::AnsiA => PageSizeKind::AnsiA,
            PageSize::AnsiB => PageSizeKind::AnsiB,
            PageSize::AnsiC => PageSizeKind::AnsiC,
            PageSize::AnsiD => PageSizeKind::AnsiD,
            PageSize::AnsiE => PageSizeKind::AnsiE,
            PageSize::UsLetter => PageSizeKind::UsLetter,
            PageSize::UsLegal => PageSizeKind::UsLegal,
            PageSize::Custom { .. } => PageSizeKind::Custom,
        }
    }
}

impl fmt::Display for Orientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Orientation::Portrait => "Portrait",
            Orientation::Landscape => "Landscape",
        })
    }
}

/// The three scale choices of the modal; `Percent` takes its value from a
/// separate input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfScaleMode {
    FitToPage,
    OneToOne,
    Percent,
}

impl PdfScaleMode {
    pub const ALL: [PdfScaleMode; 3] = [
        PdfScaleMode::FitToPage,
        PdfScaleMode::OneToOne,
        PdfScaleMode::Percent,
    ];
}

impl fmt::Display for PdfScaleMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PdfScaleMode::FitToPage => "Fit to Page",
            PdfScaleMode::OneToOne => "1:1",
            PdfScaleMode::Percent => "Percent",
        })
    }
}

impl PdfScale {
    /// The picker entry for this scale.
    pub fn mode(self) -> PdfScaleMode {
        match self {
            PdfScale::FitToPage => PdfScaleMode::FitToPage,
            PdfScale::OneToOne => PdfScaleMode::OneToOne,
            PdfScale::Percent(_) => PdfScaleMode::Percent,
        }
    }
}

/// One side of the page margin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginEdge {
    Top,
    Right,
    Bottom,
    Left,
}

impl MarginEdge {
    pub const ALL: [MarginEdge; 4] = [
        MarginEdge::Top,
        MarginEdge::Right,
        MarginEdge::Bottom,
        MarginEdge::Left,
    ];
}

impl fmt::Display for MarginEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MarginEdge::Top => "Top",
            MarginEdge::Right => "Right",
            MarginEdge::Bottom => "Bottom",
            MarginEdge::Left => "Left",
        })
    }
}

/// Result of a margin edit: the new margins and whether the value had to be
/// reduced to fit the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarginUpdate {
    pub margins: Margins,
    pub clamped: bool,
}

impl Margins {
    pub fn edge_mm(&self, edge: MarginEdge) -> f64 {
        match edge {
            MarginEdge::Top => self.top_mm,
            MarginEdge::Right => self.right_mm,
            MarginEdge::Bottom => self.bottom_mm,
            MarginEdge::Left => self.left_mm,
        }
    }

    fn set_edge_mm(&mut self, edge: MarginEdge, value: f64) {
        match edge {
            MarginEdge::Top => self.top_mm = value,
            MarginEdge::Right => self.right_mm = value,
            MarginEdge::Bottom => self.bottom_mm = value,
            MarginEdge::Left => self.left_mm = value,
        }
    }

    /// Parse `text` as `edge`'s margin and clamp it so the printable area on
    /// that axis keeps at least [`MIN_PRINTABLE_MM`] on a `page_w_mm` x
    /// `page_h_mm` page. Bad text returns an error and leaves `self` untouched.
    pub fn with_edge_text(
        &self,
        edge: MarginEdge,
        text: &str,
        page_w_mm: f64,
        page_h_mm: f64,
    ) -> Result<MarginUpdate, PageSetupError> {
        let requested = parse_mm(text)?;
        let (page_axis, opposite) = match edge {
            MarginEdge::Top => (page_h_mm, self.bottom_mm),
            MarginEdge::Bottom => (page_h_mm, self.top_mm),
            MarginEdge::Left => (page_w_mm, self.right_mm),
            MarginEdge::Right => (page_w_mm, self.left_mm),
        };
        let room = page_axis - opposite - MIN_PRINTABLE_MM;
        if room <= 0.0 {
            return Err(PageSetupError::NoRoom);
        }
        let value = requested.min(room);
        let mut margins = *self;
        margins.set_edge_mm(edge, value);
        Ok(MarginUpdate {
            margins,
            clamped: value < requested,
        })
    }

    /// Scale margins down on any axis whose pair would leave less than
    /// [`MIN_PRINTABLE_MM`] — used after the page shrinks under existing margins.
    pub fn fitted_to_page(self, page_w_mm: f64, page_h_mm: f64) -> Margins {
        fn fit_pair(a: f64, b: f64, page: f64) -> (f64, f64) {
            let allowed = (page - MIN_PRINTABLE_MM).max(0.0);
            if a + b <= allowed || a + b <= 0.0 {
                (a, b)
            } else {
                let k = allowed / (a + b);
                (a * k, b * k)
            }
        }
        let (left_mm, right_mm) = fit_pair(self.left_mm, self.right_mm, page_w_mm);
        let (top_mm, bottom_mm) = fit_pair(self.top_mm, self.bottom_mm, page_h_mm);
        Margins {
            top_mm,
            right_mm,
            bottom_mm,
            left_mm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_size_kind_round_trips_through_the_picker() {
        for kind in PageSizeKind::ALL {
            let size = kind.select(PageSize::IsoA4, Orientation::Landscape);
            assert_eq!(size.kind(), kind, "{kind}");
        }
    }

    #[test]
    fn from_standard_str_sizes_are_all_selectable() {
        for paper in ["A0", "A1", "A2", "A3", "A4", "A5", "A", "B", "C", "D", "E"] {
            let size = PageSize::from_standard_str(paper);
            assert!(PageSizeKind::ALL.contains(&size.kind()), "{paper}");
        }
        // Non-preset sheet papers arrive as Custom and stay selectable.
        let tabloid = PageSize::from_standard_str("Tabloid");
        assert_eq!(tabloid.kind(), PageSizeKind::Custom);
    }

    #[test]
    fn switching_to_custom_keeps_the_current_effective_size() {
        let custom = PageSizeKind::Custom.select(PageSize::IsoA3, Orientation::Landscape);
        assert_eq!(
            custom,
            PageSize::Custom {
                width_mm: 420.0,
                height_mm: 297.0
            }
        );
        let again = PageSizeKind::Custom.select(custom, Orientation::Portrait);
        assert_eq!(again, custom);
    }

    #[test]
    fn mm_and_percent_reject_garbage() {
        for bad in [
            "", "  ", "abc", "-5", "0", "0.0", "NaN", "inf", "-inf", "1e999",
        ] {
            assert!(parse_mm(bad).is_err(), "mm {bad:?}");
            assert!(parse_percent(bad).is_err(), "percent {bad:?}");
        }
        assert_eq!(parse_mm(" 12.5 "), Ok(12.5));
        assert_eq!(parse_percent("50%"), Ok(50.0));
        assert_eq!(
            parse_mm("9999"),
            Err(PageSetupError::TooLarge(MAX_PAGE_EDGE_MM))
        );
        assert_eq!(parse_percent("-5"), Err(PageSetupError::NotPositive));
        assert_eq!(parse_percent(""), Err(PageSetupError::Empty));
    }

    fn margins(m: f64) -> Margins {
        Margins {
            top_mm: m,
            right_mm: m,
            bottom_mm: m,
            left_mm: m,
        }
    }

    #[test]
    fn custom_edge_has_a_lower_bound() {
        for small in ["9.99", "5", "0.5"] {
            assert_eq!(
                parse_custom_edge_mm(small),
                Err(PageSetupError::TooSmall(MIN_CUSTOM_EDGE_MM)),
                "{small}"
            );
        }
        assert_eq!(parse_custom_edge_mm("10"), Ok(10.0));
        assert!(parse_custom_edge_mm("0").is_err());
        assert!(parse_custom_edge_mm("abc").is_err());
    }

    #[test]
    fn margin_edit_changes_only_its_edge() {
        let update = margins(10.0)
            .with_edge_text(MarginEdge::Left, "25", 297.0, 210.0)
            .expect("valid");
        assert_eq!(update.margins.left_mm, 25.0);
        assert_eq!(update.margins.right_mm, 10.0);
        assert!(!update.clamped);
    }

    #[test]
    fn margin_is_clamped_to_the_page() {
        let update = margins(10.0)
            .with_edge_text(MarginEdge::Top, "500", 297.0, 210.0)
            .expect("valid");
        assert_eq!(update.margins.top_mm, 210.0 - 10.0 - MIN_PRINTABLE_MM);
        assert!(update.clamped);
    }

    #[test]
    fn rejected_margin_text_leaves_margins_alone() {
        let start = margins(10.0);
        for bad in ["", "abc", "-5", "0"] {
            assert!(
                start
                    .with_edge_text(MarginEdge::Right, bad, 297.0, 210.0)
                    .is_err()
            );
        }
    }

    #[test]
    fn shrinking_the_page_refits_existing_margins() {
        let fitted = margins(40.0).fitted_to_page(60.0, 400.0);
        assert!(fitted.left_mm + fitted.right_mm <= 60.0 - MIN_PRINTABLE_MM + 1e-9);
        assert_eq!(fitted.top_mm, 40.0);
    }
}
