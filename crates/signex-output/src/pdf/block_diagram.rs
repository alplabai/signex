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

mod tables;

pub use tables::{BlockDiagramTable, BlockDiagramTableRow};

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
    /// Pages printed.
    pub page_count: usize,
}

/// One page of a block diagram's PDF, in the order printed.
#[derive(Debug, Clone, Copy)]
pub enum BlockDiagramPage<'a> {
    /// The diagram fitted on the page.
    Diagram(&'a BlockDiagramDrawing),
    /// Rows (the power tree, the budget table), on as many pages as needed.
    Table(&'a BlockDiagramTable),
}

/// `drawing` fitted on one page as `options` ask.
pub fn block_diagram_pdf(
    drawing: &BlockDiagramDrawing,
    options: &BlockDiagramPdfOptions,
    text: &BlockDiagramPdfText,
) -> Result<BlockDiagramPdfOutput, PdfError> {
    block_diagram_pdf_pages(&[BlockDiagramPage::Diagram(drawing)], options, text)
}

/// `pages` printed in order on the paper `options` names; the diagram's
/// page carries the title block when asked. `scale` is the diagram page's
/// (1 without one).
pub fn block_diagram_pdf_pages(
    pages: &[BlockDiagramPage<'_>],
    options: &BlockDiagramPdfOptions,
    text: &BlockDiagramPdfText,
) -> Result<BlockDiagramPdfOutput, PdfError> {
    let page_mm = options.page_size.dimensions_mm(options.orientation);
    let per_page = tables::rows_per_page(page_mm);
    let chunks = |table: &BlockDiagramTable| table.rows.len().div_ceil(per_page).max(1);
    let total: usize = pages
        .iter()
        .map(|page| match page {
            BlockDiagramPage::Diagram(_) => 1,
            BlockDiagramPage::Table(table) => chunks(table),
        })
        .sum();
    let mut fonts = FontUsage::new();
    let mut contents = Vec::new();
    let mut scale = 1.0;
    for page in pages {
        match page {
            BlockDiagramPage::Diagram(drawing) => {
                let at = (contents.len() + 1, total);
                let mut surface = PdfSurface::new(&mut fonts);
                scale = diagram_page(&mut surface, drawing, options, text, page_mm, at);
                contents.push(surface.finish());
            }
            BlockDiagramPage::Table(table) => {
                for at in 0..chunks(table) {
                    let end = ((at + 1) * per_page).min(table.rows.len());
                    let rows = &table.rows[(at * per_page).min(end)..end];
                    let mut surface = PdfSurface::new(&mut fonts);
                    tables::draw_table_page(&mut surface, table, rows, page_mm, options.monochrome);
                    contents.push(surface.finish());
                }
            }
        }
    }
    let bytes = assemble(&contents, &fonts, page_mm.0, page_mm.1)?;
    Ok(BlockDiagramPdfOutput {
        bytes,
        scale,
        uncovered_chars: fonts.uncovered_chars(),
        page_count: contents.len(),
    })
}

/// Draw the diagram's page; answers its scale.
fn diagram_page(
    surface: &mut PdfSurface<'_>,
    drawing: &BlockDiagramDrawing,
    options: &BlockDiagramPdfOptions,
    text: &BlockDiagramPdfText,
    page_mm: (f64, f64),
    page: (usize, usize),
) -> f64 {
    let frame = PageFrame::place(
        (drawing.min, drawing.max),
        BoardPrintScale::Fit,
        options.title_block,
        page_mm,
        false,
    );
    // Round caps and joins, so a bent connection has no notch.
    surface.raw_operator("1 J 1 j\n");
    for mark in &drawing.marks {
        draw_mark(surface, &frame, mark);
    }
    if options.title_block {
        let block = TitleBlock {
            title: &text.title,
            label: &text.label,
            scale: frame.scale(),
            page,
        };
        title_block(surface, &frame, &block, options.monochrome);
    }
    frame.scale()
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
