//! Where the board lands on a printed page: its extent, the scale, the
//! centring inside the margins and above the title block, and the mirror.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_origin::EMPTY_BOARD_AREA;

use super::super::MM_TO_PT;
use super::{BoardPrintOptions, BoardPrintScale};

/// Blank paper around the drawing, mm.
pub(super) const MARGIN_MM: f64 = 10.0;
/// Height of the title block under the drawing, mm.
pub(super) const TITLE_BLOCK_MM: f64 = 18.0;

/// The page transform of one printed page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PageFrame {
    /// The board extent, model mm (y down).
    min: Point,
    max: Point,
    /// Page mm per board mm.
    scale: f64,
    /// Where the extent's first corner lands, page mm from the top-left.
    at_mm: (f64, f64),
    mirrored: bool,
    page_mm: (f64, f64),
}

impl PageFrame {
    pub(crate) fn new(
        board: &PcbBoard,
        options: &BoardPrintOptions,
        page_mm: (f64, f64),
        mirrored: bool,
    ) -> Self {
        Self::for_extent(board_extent(board), options, page_mm, mirrored)
    }

    /// The frame of a drawing over `extent` (model mm) rather than the
    /// board's outline: an assembly drawing framed on its Edge Cuts.
    pub(crate) fn for_extent(
        extent: (Point, Point),
        options: &BoardPrintOptions,
        page_mm: (f64, f64),
        mirrored: bool,
    ) -> Self {
        Self::place(
            extent,
            options.scale,
            options.title_block,
            page_mm,
            mirrored,
        )
    }

    /// The frame of any drawing over `extent` (model mm) at `scale`, room
    /// left under it for a title block when `title_block`: a block diagram
    /// page as well as a board's.
    pub(crate) fn place(
        (min, max): (Point, Point),
        scale: BoardPrintScale,
        title_block: bool,
        page_mm: (f64, f64),
        mirrored: bool,
    ) -> Self {
        let (w, h) = ((max.x - min.x).max(1e-6), (max.y - min.y).max(1e-6));
        let below = if title_block {
            TITLE_BLOCK_MM + MARGIN_MM / 2.0
        } else {
            0.0
        };
        let area = (
            (page_mm.0 - 2.0 * MARGIN_MM).max(1.0),
            (page_mm.1 - 2.0 * MARGIN_MM - below).max(1.0),
        );
        let scale = match scale {
            BoardPrintScale::OneToOne => 1.0,
            BoardPrintScale::Fit => (area.0 / w).min(area.1 / h),
        };
        let at_mm = (
            MARGIN_MM + (area.0 - w * scale) / 2.0,
            MARGIN_MM + (area.1 - h * scale) / 2.0,
        );
        Self {
            min,
            max,
            scale,
            at_mm,
            mirrored,
            page_mm,
        }
    }

    pub(crate) fn scale(&self) -> f64 {
        self.scale
    }

    pub(crate) fn mirrored(&self) -> bool {
        self.mirrored
    }

    /// The board extent as printed, page mm.
    pub(crate) fn printed_size_mm(&self) -> (f64, f64) {
        (
            (self.max.x - self.min.x) * self.scale,
            (self.max.y - self.min.y) * self.scale,
        )
    }

    /// Whether the board runs past the margins: at 1:1, a board larger than
    /// the printable area. A fitted board never does.
    pub(crate) fn overflows(&self) -> bool {
        const SLACK_MM: f64 = 1e-6;
        self.at_mm.0 < MARGIN_MM - SLACK_MM || self.at_mm.1 < MARGIN_MM - SLACK_MM
    }

    /// A board point (model mm) on the page, PDF points from the
    /// bottom-left.
    pub(crate) fn to_pt(self, x_mm: f64, y_mm: f64) -> (f32, f32) {
        let u = if self.mirrored {
            self.max.x - x_mm
        } else {
            x_mm - self.min.x
        };
        let v = y_mm - self.min.y;
        let page_x = self.at_mm.0 + u * self.scale;
        let page_y = self.at_mm.1 + v * self.scale;
        (
            (page_x * MM_TO_PT) as f32,
            ((self.page_mm.1 - page_y) * MM_TO_PT) as f32,
        )
    }

    /// A board length (mm) on the page, PDF points.
    pub(crate) fn length_pt(&self, mm: f64) -> f32 {
        (mm * self.scale * MM_TO_PT) as f32
    }

    /// The page size, PDF points.
    pub(crate) fn page_pt(&self) -> (f32, f32) {
        (
            (self.page_mm.0 * MM_TO_PT) as f32,
            (self.page_mm.1 * MM_TO_PT) as f32,
        )
    }
}

/// The board's extent: its outline's, or the default board area of a
/// board with no outline yet.
fn board_extent(board: &PcbBoard) -> (Point, Point) {
    let points: Vec<&Point> = board
        .outline
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect();
    if points.len() < 3 {
        return EMPTY_BOARD_AREA;
    }
    points.iter().fold(
        (
            Point::new(f64::INFINITY, f64::INFINITY),
            Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        ),
        |(min, max), p| {
            (
                Point::new(min.x.min(p.x), min.y.min(p.y)),
                Point::new(max.x.max(p.x), max.y.max(p.y)),
            )
        },
    )
}
