//! A plain flowing-text PDF (headings and body lines on A4 portrait pages),
//! for reports that are text, not a drawing: the ERC report (#1287). Uses
//! the same embedded fonts and content-stream surface as the schematic PDF.

use pdf_writer::{Finish, Name, Pdf, Rect, Ref};

use super::embed::{FontUsage, write_fonts};
use super::font::{PdfFont, best_alias_for_text, normalize_pdf_text, text_advance_pt};
use super::surface::PdfSurface;
use super::{MM_TO_PT, PdfError};

const PAGE_W_MM: f64 = 210.0;
const PAGE_H_MM: f64 = 297.0;
const MARGIN_PT: f32 = 42.0;

/// How a line is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineStyle {
    Title,
    Heading,
    Body,
    /// Body text, indented.
    Detail,
}

impl LineStyle {
    fn size_pt(self) -> f32 {
        match self {
            Self::Title => 16.0,
            Self::Heading => 11.0,
            Self::Body | Self::Detail => 8.5,
        }
    }

    fn alias(self) -> &'static str {
        match self {
            Self::Title | Self::Heading => PdfFont::RobotoBold.alias(),
            Self::Body | Self::Detail => PdfFont::RobotoRegular.alias(),
        }
    }

    fn indent_pt(self) -> f32 {
        if self == Self::Detail { 14.0 } else { 0.0 }
    }

    /// Space above the line, so headings stand apart.
    fn gap_before_pt(self) -> f32 {
        match self {
            Self::Title => 0.0,
            Self::Heading => 8.0,
            Self::Body | Self::Detail => 0.0,
        }
    }
}

/// One logical line; long lines wrap to the page width.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextLine {
    pub style: LineStyle,
    pub text: String,
}

/// Lay `lines` out on as many pages as they need and return the PDF bytes.
pub(crate) fn render(lines: &[TextLine]) -> Result<Vec<u8>, PdfError> {
    let page_w = (PAGE_W_MM * MM_TO_PT) as f32;
    let page_h = (PAGE_H_MM * MM_TO_PT) as f32;
    let mut fonts = FontUsage::new();
    let pages = lay_out(lines, page_w, page_h, &mut fonts);
    // Two refs per page plus the fonts must fit an i32 ref id.
    if pages.len() > (i32::MAX as usize) / 4 {
        return Err(PdfError::TooManyPages(pages.len()));
    }
    let n = pages.len() as i32;
    let (catalog_id, tree_id) = (Ref::new(1), Ref::new(2));
    let page_refs: Vec<Ref> = (0..n).map(|i| Ref::new(3 + i)).collect();
    let content_refs: Vec<Ref> = (0..n).map(|i| Ref::new(3 + n + i)).collect();
    let font_base = 3 + 2 * n;

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id).kids(page_refs.iter().copied()).count(n);
    let embedded = write_fonts(&mut pdf, &fonts, font_base)?;
    for (i, content) in pages.iter().enumerate() {
        pdf.stream(content_refs[i], content);
        let mut page = pdf.page(page_refs[i]);
        page.parent(tree_id)
            .media_box(Rect::new(0.0, 0.0, page_w, page_h))
            .contents(content_refs[i]);
        let mut resources = page.resources();
        let mut font_dict = resources.fonts();
        for f in &embedded {
            font_dict.pair(Name(f.font.alias().as_bytes()), f.type0);
        }
        font_dict.finish();
        resources.finish();
        page.finish();
    }
    Ok(pdf.finish())
}

/// One placed run of text.
struct Placed {
    x: f32,
    y: f32,
    alias: &'static str,
    size: f32,
    text: String,
}

/// Content streams, one per page. At least one page, even with no lines.
fn lay_out(lines: &[TextLine], page_w: f32, page_h: f32, fonts: &mut FontUsage) -> Vec<Vec<u8>> {
    paginate(lines, page_w, page_h)
        .into_iter()
        .map(|page| {
            let mut surface = PdfSurface::new(&mut *fonts);
            for p in page {
                surface.text_at(p.x, p.y, p.alias, p.size, &p.text);
            }
            surface.finish()
        })
        .collect()
}

/// Wrap and place every line, starting a new page when the bottom margin is
/// reached.
fn paginate(lines: &[TextLine], page_w: f32, page_h: f32) -> Vec<Vec<Placed>> {
    let mut pages: Vec<Vec<Placed>> = vec![Vec::new()];
    let mut y = page_h - MARGIN_PT;
    for line in lines {
        let size = line.style.size_pt();
        let leading = size * 1.35;
        let x = MARGIN_PT + line.style.indent_pt();
        let width = page_w - MARGIN_PT - x;
        let text = normalize_pdf_text(&line.text);
        let alias = best_alias_for_text(line.style.alias(), &text);
        y -= line.style.gap_before_pt();
        for piece in wrap(&text, alias, size, width) {
            if y - leading < MARGIN_PT {
                pages.push(Vec::new());
                y = page_h - MARGIN_PT;
            }
            y -= leading;
            if let Some(page) = pages.last_mut() {
                page.push(Placed {
                    x,
                    y,
                    alias,
                    size,
                    text: piece,
                });
            }
        }
    }
    pages
}

/// Greedy word wrap to `width` points; a word wider than the line is split
/// by characters. An empty line stays one empty piece.
fn wrap(text: &str, alias: &str, size: f32, width: f32) -> Vec<String> {
    let fits = |s: &str| text_advance_pt(alias, s, size) <= width;
    let mut out = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if fits(&candidate) {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        for ch in word.chars() {
            current.push(ch);
            if !fits(&current) && current.chars().count() > 1 {
                current.pop();
                out.push(std::mem::take(&mut current));
                current.push(ch);
            }
        }
    }
    out.push(current);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_wrap_inside_the_width_and_long_words_split() {
        let text = "word ".repeat(200);
        let pieces = wrap(text.trim(), "F1", 8.5, 300.0);
        assert!(pieces.len() > 1);
        assert!(
            pieces
                .iter()
                .all(|p| text_advance_pt("F1", p, 8.5) <= 300.0)
        );
        let word = "x".repeat(500);
        let split = wrap(&word, "F1", 8.5, 100.0);
        assert!(split.len() > 1);
        assert_eq!(split.concat(), word);
    }

    #[test]
    fn many_lines_flow_onto_several_pages() {
        let lines: Vec<TextLine> = (0..300)
            .map(|i| TextLine {
                style: LineStyle::Body,
                text: format!("line {i}"),
            })
            .collect();
        let pages = paginate(&lines, 595.0, 842.0);
        assert!(pages.len() > 1);
        assert_eq!(pages.iter().map(Vec::len).sum::<usize>(), 300);
        let bytes = render(&lines).expect("pdf");
        assert!(bytes.starts_with(b"%PDF-"));
    }
}
