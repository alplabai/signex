//! A table row that opens a sheet (the contents table, #2663) becomes a
//! `/Link` annotation over the row, going to the page that prints that
//! sheet. See docs/PDF_EXPORT.md, "Table of contents links".

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pdf_writer::types::{ActionType, AnnotationType};
use pdf_writer::{Finish, Pdf, Rect, Ref};
use signex_types::lexical_path::lexically_normalize;
use signex_types::schematic::SchDrawing;

use super::layout::PageTransform;
use super::page_range::resolve_page_range;
use super::{MM_TO_PT, PdfError, PdfOptions};
use crate::ExportContext;

/// One table row written as a link.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfTableLink {
    /// 0-based exported page the row is drawn on.
    pub page: usize,
    /// 0-based body row of its table.
    pub row: usize,
    /// The row's stored link, relative to the folder of the sheet holding
    /// the table.
    pub link: String,
    /// 0-based exported page the link goes to.
    pub target_page: usize,
    /// The row's box in PDF points: left, bottom, right, top.
    pub rect_pt: [f32; 4],
}

/// The links the PDF of `ctx` with `opts` carries, in page and row order:
/// what [`super::PdfExporter`] writes, without building the pages. None
/// with `opts.links` off.
pub fn pdf_table_links(
    ctx: &ExportContext,
    opts: &PdfOptions,
) -> Result<Vec<PdfTableLink>, PdfError> {
    if ctx.sheets.is_empty() {
        return Err(PdfError::NoSheets);
    }
    let pages = resolve_page_range(&opts.page_range, &ctx.sheets, ctx.current_sheet_index())?;
    if !opts.links {
        return Ok(Vec::new());
    }
    let (page_w_mm, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let page_h_pt = (page_h_mm * MM_TO_PT) as f32;
    Ok(collect(ctx, opts, &pages, page_w_mm, page_h_mm, page_h_pt))
}

/// Every row with a link to an exported sheet, on every exported page. A
/// link to a sheet no exported page prints is left as plain text. When
/// several pages print the same file (the channels of a repeated sheet), the
/// table's n-th row naming that file goes to the n-th such page, and a row
/// past the last goes to the first.
pub(super) fn collect(
    ctx: &ExportContext,
    opts: &PdfOptions,
    page_sheet_indices: &[usize],
    page_w_mm: f64,
    page_h_mm: f64,
    page_h_pt: f32,
) -> Vec<PdfTableLink> {
    let page_files: Vec<Option<PathBuf>> = page_sheet_indices
        .iter()
        .map(|&i| ctx.sheets.get(i).map(|s| lexically_normalize(&s.path)))
        .collect();
    let pages_printing = |file: &Path| -> Vec<usize> {
        (0..page_files.len())
            .filter(|&p| page_files[p].as_deref() == Some(file))
            .collect()
    };
    let mut out = Vec::new();
    for (page, &sheet_idx) in page_sheet_indices.iter().enumerate() {
        let Some(sheet) = ctx.sheets.get(sheet_idx) else {
            continue;
        };
        let folder = sheet.path.parent().unwrap_or(Path::new(""));
        let xform = PageTransform::new(sheet, page_w_mm, page_h_mm, opts, MM_TO_PT);
        for drawing in &sheet.schematic.drawings {
            let SchDrawing::Table(table) = drawing else {
                continue;
            };
            let layout = table.layout();
            if layout.width <= 0.0 {
                continue;
            }
            let mut named: HashMap<PathBuf, usize> = HashMap::new();
            for row in 0..layout.body_rows {
                let Some(link) = table.link(row) else {
                    continue;
                };
                let target = lexically_normalize(&folder.join(link.trim()));
                let nth = named.entry(target.clone()).or_insert(0);
                let targets = pages_printing(&target);
                let Some(&target_page) = targets.get(*nth).or(targets.first()) else {
                    continue;
                };
                *nth += 1;
                let top = layout.origin.y + (row + 1) as f64 * layout.row_height;
                let bottom = top + layout.row_height;
                out.push(PdfTableLink {
                    page,
                    row,
                    link: link.to_string(),
                    target_page,
                    rect_pt: [
                        xform.x(layout.origin.x),
                        xform.pdf_y(bottom, page_h_pt),
                        xform.x(layout.origin.x + layout.width),
                        xform.pdf_y(top, page_h_pt),
                    ],
                });
            }
        }
    }
    out
}

/// The annotation refs of the links on `page`, for its `/Annots` array.
/// Link `i` is object `first_ref + i`.
pub(super) fn page_annotation_refs(
    links: &[PdfTableLink],
    page: usize,
    first_ref: i32,
) -> Vec<Ref> {
    links
        .iter()
        .enumerate()
        .filter(|(_, link)| link.page == page)
        .map(|(i, _)| Ref::new(first_ref + i as i32))
        .collect()
}

/// Write every link as a borderless `/Link` annotation whose GoTo action
/// shows the whole target page.
pub(super) fn write_annotations(
    pdf: &mut Pdf,
    links: &[PdfTableLink],
    first_ref: i32,
    page_refs: &[Ref],
) {
    for (i, link) in links.iter().enumerate() {
        let target = page_refs[link.target_page];
        let [left, bottom, right, top] = link.rect_pt;
        let mut annotation = pdf.annotation(Ref::new(first_ref + i as i32));
        annotation
            .subtype(AnnotationType::Link)
            .rect(Rect::new(left, bottom, right, top))
            .border(0.0, 0.0, 0.0, None);
        annotation
            .action()
            .action_type(ActionType::GoTo)
            .destination()
            .page(target)
            .fit();
        annotation.finish();
    }
}

#[cfg(test)]
#[path = "table_links_tests.rs"]
mod tests;
