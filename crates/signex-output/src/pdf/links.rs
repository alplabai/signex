//! PDF link annotations (#1298): ports jump to their partner, sheet symbols
//! and their entries to the child page, a part with a web datasheet opens
//! it, and a `[label](url)` link in text opens its address. The rules are
//! in docs/PDF_EXPORT.md, "Links".

use std::path::Path;

use pdf_writer::types::{ActionType, AnnotationType};
use pdf_writer::{Finish, Pdf, Rect, Ref, Str};
use signex_types::schematic::{Aabb, LabelType, Point, body_bounds, text_aabb, text_height_mm};

use super::layout::PageTransform;
use super::surface::PageContent;
use super::{MM_TO_PT, PdfOptions};
use crate::{ExportContext, SheetSnapshot};

/// Where a link goes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LinkTarget {
    /// An address a reader opens outside the PDF.
    Uri(String),
    /// A point on an exported page, in PDF points (y up).
    Page {
        page_idx: usize,
        x_pt: f32,
        y_pt: f32,
    },
}

/// One clickable area of a page, in PDF points (y up).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageLink {
    /// Left, bottom, right, top.
    pub rect: [f32; 4],
    pub target: LinkTarget,
}

impl PageLink {
    /// A link over the page-space box `(x1, y1)`..`(x2, y2)`, in any corner
    /// order.
    pub(crate) fn over(x1: f32, y1: f32, x2: f32, y2: f32, target: LinkTarget) -> Self {
        Self {
            rect: [x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)],
            target,
        }
    }
}

/// The schemes a link may open. Anything else (a file path, `javascript:`,
/// a local program) gets no link.
const SCHEMES: [&str; 3] = ["https://", "http://", "mailto:"];

/// `text` as an address a link may open: a web or mail address, ASCII, with
/// no space or control character. `None` for anything else.
pub(crate) fn linkable_uri(text: &str) -> Option<&str> {
    let uri = text.trim();
    let lower = uri.to_ascii_lowercase();
    let scheme = SCHEMES.iter().find(|s| lower.starts_with(*s))?;
    let has_rest = uri.len() > scheme.len();
    let clean = uri.is_ascii() && !uri.bytes().any(|b| b.is_ascii_whitespace() || b < 0x20);
    (has_rest && clean).then_some(uri)
}

/// One exported page and how its sheet sits on it.
struct Page<'a> {
    sheet: &'a SheetSnapshot,
    xform: PageTransform,
}

/// The exported pages, to find a link's partner among them.
pub(crate) struct LinkPages<'a> {
    pages: Vec<Page<'a>>,
    page_h_pt: f32,
}

impl<'a> LinkPages<'a> {
    pub(crate) fn new(
        ctx: &'a ExportContext,
        opts: &PdfOptions,
        sheet_indices: &[usize],
        (page_w_mm, page_h_mm): (f64, f64),
    ) -> Self {
        let pages = sheet_indices
            .iter()
            .filter_map(|&i| ctx.sheets.get(i))
            .map(|sheet| Page {
                sheet,
                xform: PageTransform::new(sheet, page_w_mm, page_h_mm, opts, MM_TO_PT),
            })
            .collect();
        Self {
            pages,
            page_h_pt: (page_h_mm * MM_TO_PT) as f32,
        }
    }

    /// Every link drawn from the sheet of page `page_idx`, bottom to top:
    /// sheet symbols, their entries, parts, then ports and global labels.
    pub(crate) fn sheet_links(&self, page_idx: usize) -> Vec<PageLink> {
        let Some(page) = self.pages.get(page_idx) else {
            return Vec::new();
        };
        let schematic = &page.sheet.schematic;
        let mut links = Vec::new();
        for child in &schematic.child_sheets {
            let Some(child_idx) = self.child_page(page_idx, child.uuid, &child.filename) else {
                continue;
            };
            let (x, y) = (child.position.x, child.position.y);
            let body = Aabb::new(x, y, x + child.size.0, y + child.size.1);
            links.push(self.link_over(page, body, self.page_top(child_idx)));
            for pin in &child.pins {
                let target = self
                    .port_on(child_idx, LabelType::Hierarchical, pin.name.trim())
                    .unwrap_or_else(|| self.page_top(child_idx));
                // A square round the entry, one text height each way.
                let (px, py) = (pin.position.x, pin.position.y);
                let around = Aabb::new(px, py, px, py).expand(text_height_mm(0.0));
                links.push(self.link_over(page, around, target));
            }
        }
        for symbol in &schematic.symbols {
            let Some(uri) = linkable_uri(&symbol.datasheet) else {
                continue;
            };
            let Some(body) = schematic
                .resolve_lib_symbol(symbol)
                .and_then(|lib| body_bounds(&lib, symbol))
            else {
                continue;
            };
            links.push(self.link_over(page, body, LinkTarget::Uri(uri.to_owned())));
        }
        for label in &schematic.labels {
            let Some(name) = label.port_name() else {
                continue;
            };
            let target = match label.label_type {
                LabelType::Hierarchical => self.parent_entry(page_idx, name),
                _ => self.next_global(page_idx, name),
            };
            let Some(target) = target else {
                continue;
            };
            let height = text_height_mm(label.font_size);
            let text = text_aabb(
                &label.text,
                label.font_size,
                (label.position.x, label.position.y),
                label.rotation,
                label.justify,
                label.justify_v,
                label.font_family.as_deref(),
            )
            .expand(height / 2.0);
            links.push(self.link_over(page, text, target));
        }
        links
    }

    /// A link over the sheet-space box `area` of `page`.
    fn link_over(&self, page: &Page<'_>, area: Aabb, target: LinkTarget) -> PageLink {
        let xform = &page.xform;
        PageLink::over(
            xform.x(area.min_x),
            xform.pdf_y(area.max_y, self.page_h_pt),
            xform.x(area.max_x),
            xform.pdf_y(area.min_y, self.page_h_pt),
            target,
        )
    }

    /// The top-left corner of page `page_idx`, where a sheet bookmark lands.
    fn page_top(&self, page_idx: usize) -> LinkTarget {
        LinkTarget::Page {
            page_idx,
            x_pt: 0.0,
            y_pt: self.page_h_pt,
        }
    }

    /// `at` on the sheet of page `page_idx`.
    fn point_on(&self, page_idx: usize, at: Point) -> LinkTarget {
        let xform = &self.pages[page_idx].xform;
        LinkTarget::Page {
            page_idx,
            x_pt: xform.x(at.x),
            y_pt: xform.pdf_y(at.y, self.page_h_pt),
        }
    }

    /// The page printing the sheet that the sheet symbol `child_uuid` on
    /// page `parent_idx` places: the page at that placement, else a page of
    /// the file the symbol names.
    fn child_page(&self, parent_idx: usize, child_uuid: uuid::Uuid, file: &str) -> Option<usize> {
        let parent = self.pages[parent_idx].sheet;
        let placement = parent.placement.child(child_uuid);
        if let Some(idx) = self
            .pages
            .iter()
            .position(|p| p.sheet.placement == placement)
        {
            return Some(idx);
        }
        let path = parent.path.parent().unwrap_or(Path::new("")).join(file);
        self.pages.iter().position(|p| p.sheet.path == path)
    }

    /// The first `kind` label named `name` on the sheet of page `page_idx`.
    fn port_on(&self, page_idx: usize, kind: LabelType, name: &str) -> Option<LinkTarget> {
        let label = self.pages[page_idx]
            .sheet
            .schematic
            .labels
            .iter()
            .find(|l| l.label_type == kind && l.port_name() == Some(name))?;
        Some(self.point_on(page_idx, label.position))
    }

    /// The sheet entry named `name` on the sheet symbol that places the sheet
    /// of page `page_idx`, else that sheet symbol. `None` when no exported
    /// page carries the symbol.
    fn parent_entry(&self, page_idx: usize, name: &str) -> Option<LinkTarget> {
        let (parent_idx, child) = self.pages.iter().enumerate().find_map(|(idx, page)| {
            let child = page
                .sheet
                .schematic
                .child_sheets
                .iter()
                .find(|c| self.child_page(idx, c.uuid, &c.filename) == Some(page_idx))?;
            Some((idx, child))
        })?;
        let at = child
            .pins
            .iter()
            .find(|pin| pin.name.trim() == name)
            .map_or(child.position, |pin| pin.position);
        Some(self.point_on(parent_idx, at))
    }

    /// The next exported page after `page_idx`, wrapping round, with a global
    /// label named `name`, at that label. `None` when no other page has one.
    fn next_global(&self, page_idx: usize, name: &str) -> Option<LinkTarget> {
        let count = self.pages.len();
        (1..count)
            .map(|step| (page_idx + step) % count)
            .find_map(|idx| self.port_on(idx, LabelType::Global, name))
    }
}

/// Every page's links, bottom to top: the sheet's own, then the text links
/// its content drew (taken out of `contents`). None at all with
/// `opts.links` off.
pub(crate) fn page_links(
    ctx: &ExportContext,
    opts: &PdfOptions,
    sheet_indices: &[usize],
    page_mm: (f64, f64),
    contents: &mut [PageContent],
) -> Vec<Vec<PageLink>> {
    if !opts.links {
        return contents.iter().map(|_| Vec::new()).collect();
    }
    let pages = LinkPages::new(ctx, opts, sheet_indices, page_mm);
    contents
        .iter_mut()
        .enumerate()
        .map(|(idx, content)| {
            let mut links = pages.sheet_links(idx);
            links.append(&mut content.links);
            links
        })
        .collect()
}

/// Write the link annotations of every page, page `i`'s at `refs[i]`.
pub(crate) fn write_annotations(
    pdf: &mut Pdf,
    links: &[Vec<PageLink>],
    refs: &[Vec<Ref>],
    page_refs: &[Ref],
) {
    for (page_links, page_annots) in links.iter().zip(refs) {
        for (link, &id) in page_links.iter().zip(page_annots) {
            let [x1, y1, x2, y2] = link.rect;
            let mut annot = pdf.annotation(id);
            annot
                .subtype(AnnotationType::Link)
                .rect(Rect::new(x1, y1, x2, y2))
                .border(0.0, 0.0, 0.0, None);
            match &link.target {
                LinkTarget::Uri(uri) => {
                    annot
                        .action()
                        .action_type(ActionType::Uri)
                        .uri(Str(uri.as_bytes()));
                }
                LinkTarget::Page {
                    page_idx,
                    x_pt,
                    y_pt,
                } => {
                    if let Some(&page) = page_refs.get(*page_idx) {
                        annot
                            .action()
                            .action_type(ActionType::GoTo)
                            .destination()
                            .page(page)
                            .xyz(*x_pt, *y_pt, None);
                    }
                }
            }
            annot.finish();
        }
    }
}

/// Number the annotations from `first_ref` on, page by page.
pub(crate) fn assign_refs(links: &[Vec<PageLink>], first_ref: i32) -> Vec<Vec<Ref>> {
    let mut next = first_ref;
    links
        .iter()
        .map(|page| {
            page.iter()
                .map(|_| {
                    let id = Ref::new(next);
                    next += 1;
                    id
                })
                .collect()
        })
        .collect()
}
