//! Cross-reference marks on an exported page, and the links that make each
//! entry jump to its page (#1273). The marks are laid out by
//! [`crate::cross_ref`], the same layout the canvas draws.

use pdf_writer::types::{ActionType, AnnotationType};
use pdf_writer::{Pdf, Rect, Ref};

use super::layout::PageTransform;
use super::*;
use crate::cross_ref::{CrossRefMark, lower_cross_refs};

/// Where a link leaves the reader: this far above and left of the partner
/// label, in mm, so the label is not on the window edge.
const LANDING_MARGIN_MM: f64 = 15.0;

/// The marks of `sheet`'s page, from the options' cross references.
pub(crate) fn page_marks(sheet: &crate::SheetSnapshot, opts: &PdfOptions) -> Vec<CrossRefMark> {
    opts.port_cross_refs
        .for_page(&sheet.path, Some(&sheet.placement))
        .map(|page| lower_cross_refs(&sheet.schematic, page))
        .unwrap_or_default()
}

/// The marks of `sheet`'s page as page elements, in the frame's ink and
/// placed with the content's transform.
pub(crate) fn cross_ref_elements(
    sheet: &crate::SheetSnapshot,
    opts: &PdfOptions,
    page_w_mm: f64,
    page_h_mm: f64,
    units_per_mm: f64,
) -> Vec<SvgElement> {
    let marks = page_marks(sheet, opts);
    if marks.is_empty() {
        return Vec::new();
    }
    let primitives: Vec<_> = marks.iter().map(CrossRefMark::frame_text).collect();
    let xform = PageTransform::new(sheet, page_w_mm, page_h_mm, opts, units_per_mm);
    super::frame::frame_elements(&primitives, &xform, opts.palette.sheet_border)
}

/// One link to write: its box on the page (PDF points, Y up: left, bottom,
/// right, top), the exported page it leads to and the point it lands on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageLink {
    pub(crate) rect: (f32, f32, f32, f32),
    /// Index among the exported pages.
    pub(crate) to_page: usize,
    pub(crate) to: (f32, f32),
}

/// The links of every exported page, in page order. An entry whose page is
/// not exported has no link; its text is still printed.
pub(crate) fn page_links(
    ctx: &ExportContext,
    opts: &PdfOptions,
    sheet_indices: &[usize],
    page_w_mm: f64,
    page_h_mm: f64,
) -> Vec<Vec<PageLink>> {
    let page_h_pt = (page_h_mm * MM_TO_PT) as f32;
    let exported = |number: usize| {
        sheet_indices
            .iter()
            .position(|&i| ctx.sheets[i].sheet_number == number)
    };
    sheet_indices
        .iter()
        .map(|&i| {
            let sheet = &ctx.sheets[i];
            let marks = page_marks(sheet, opts);
            if marks.is_empty() {
                return Vec::new();
            }
            let here = PageTransform::new(sheet, page_w_mm, page_h_mm, opts, MM_TO_PT);
            let mut links = Vec::new();
            for link in marks.iter().flat_map(|mark| &mark.links) {
                let Some(to_page) = exported(link.page) else {
                    continue;
                };
                let target = &ctx.sheets[sheet_indices[to_page]];
                let there = PageTransform::new(target, page_w_mm, page_h_mm, opts, MM_TO_PT);
                let (tx, ty) = link.target_mm;
                links.push(PageLink {
                    rect: (
                        here.x(link.x0),
                        here.pdf_y(link.y1, page_h_pt),
                        here.x(link.x1),
                        here.pdf_y(link.y0, page_h_pt),
                    ),
                    to_page,
                    to: (
                        there.x(tx - LANDING_MARGIN_MM).max(0.0),
                        there
                            .pdf_y(ty - LANDING_MARGIN_MM, page_h_pt)
                            .min(page_h_pt),
                    ),
                });
            }
            links
        })
        .collect()
}

/// Write `links` as link annotations numbered from `first`; the refs to list
/// in the page's `/Annots`.
pub(crate) fn write_links(
    pdf: &mut Pdf,
    links: &[PageLink],
    first: i32,
    page_refs: &[Ref],
) -> Vec<Ref> {
    let mut written = Vec::with_capacity(links.len());
    for (offset, link) in links.iter().enumerate() {
        let Some(&page) = page_refs.get(link.to_page) else {
            continue;
        };
        let id = Ref::new(first + offset as i32);
        let (left, bottom, right, top) = link.rect;
        let mut annotation = pdf.annotation(id);
        annotation
            .subtype(AnnotationType::Link)
            .rect(Rect::new(left, bottom, right, top))
            .border(0.0, 0.0, 0.0, None);
        annotation
            .action()
            .action_type(ActionType::GoTo)
            .destination()
            .page(page)
            .xyz(link.to.0, link.to.1, None);
        annotation.finish();
        written.push(id);
    }
    written
}
