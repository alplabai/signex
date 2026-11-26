//! A block diagram on a printed page (#2796, mockup row 171): the marks of
//! [`crate::block_diagram`] fitted to the paper, with a title block, in the
//! same embedded fonts and content-stream surface as the board PDF. Rules:
//! `docs/BLOCK_DIAGRAM_EXPORT.md`.

use signex_types::schematic::SheetOrientation;

use super::board::{PageFrame, TitleBlock, assemble, fill, label, stroke, title_block};
use super::embed::FontUsage;
use super::surface::PdfSurface;
use super::{PageSize, PdfError};
use crate::BoardPrintScale;
use crate::block_diagram::{BlockDiagramDrawing, BlockMark};

/// How a block diagram is printed. The default is A4 landscape with a
/// title block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockDiagramPdfOptions {
    pub page_size: PageSize,
    pub orientation: SheetOrientation,
    pub title_block: bool,
    /// The title block in black too; the drawing's own colours come from
    /// its [`crate::block_diagram::BlockDiagramLook`].
    pub monochrome: bool,
}

impl Default for BlockDiagramPdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSize::IsoA4,
            orientation: SheetOrientation::Landscape,
            title_block: true,
            monochrome: false,
        }
    }
}

/// The words the title block prints, in the user's language: the caller's,
/// since this crate has no message catalog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockDiagramPdfText {
    /// The diagram's name, usually its file name.
    pub title: String,
    /// What the page shows ("Block diagram").
    pub label: String,
}

/// A printed block diagram.
#[derive(Debug, Clone)]
pub struct BlockDiagramPdfOutput {
    pub bytes: Vec<u8>,
    /// Page mm per diagram mm.
    pub scale: f64,
    /// Characters no bundled font covers, sorted and unique.
    pub uncovered_chars: Vec<char>,
}

/// `drawing` fitted on one page as `options` ask.
pub fn block_diagram_pdf(
    drawing: &BlockDiagramDrawing,
    options: &BlockDiagramPdfOptions,
    text: &BlockDiagramPdfText,
) -> Result<BlockDiagramPdfOutput, PdfError> {
    let page_mm = options.page_size.dimensions_mm(options.orientation);
    let frame = PageFrame::place(
        (drawing.min, drawing.max),
        BoardPrintScale::Fit,
        options.title_block,
        page_mm,
        false,
    );
    let mut fonts = FontUsage::new();
    let mut surface = PdfSurface::new(&mut fonts);
    // Round caps and joins, so a bent connection has no notch.
    surface.raw_operator("1 J 1 j\n");
    for mark in &drawing.marks {
        draw_mark(&mut surface, &frame, mark);
    }
    if options.title_block {
        let block = TitleBlock {
            title: &text.title,
            label: &text.label,
            scale: frame.scale(),
            page: (1, 1),
        };
        title_block(&mut surface, &frame, &block, options.monochrome);
    }
    let content = surface.finish();
    let bytes = assemble(&[content], &fonts, page_mm.0, page_mm.1)?;
    Ok(BlockDiagramPdfOutput {
        bytes,
        scale: frame.scale(),
        uncovered_chars: fonts.uncovered_chars(),
    })
}

fn draw_mark(s: &mut PdfSurface<'_>, frame: &PageFrame, mark: &BlockMark) {
    let ring = |points: &[signex_types::schematic::Point]| -> Vec<(f32, f32)> {
        points.iter().map(|p| frame.to_pt(p.x, p.y)).collect()
    };
    match mark {
        BlockMark::Stroke {
            points,
            closed,
            width_mm,
            rgb,
        } => stroke(
            s,
            &ring(points),
            *closed,
            frame.length_pt(width_mm.max(0.0)),
            *rgb,
        ),
        BlockMark::Fill { ring: points, rgb } => fill(s, &[ring(points)], *rgb),
        BlockMark::Text {
            at,
            size_mm,
            text,
            rgb,
        } => label(s, frame, (at.x, at.y), 0.0, text, *size_mm, *rgb),
    }
}

#[cfg(test)]
mod tests;
