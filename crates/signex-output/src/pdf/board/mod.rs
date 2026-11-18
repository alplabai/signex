//! Board print and PDF (#2402): the board's layers drawn to scale on paper,
//! one page per chosen layer set or every set on one composite page, the
//! bottom side mirrored if asked, 1:1 or fitted, in colour or black, with
//! the outline, drill holes and a title block, and the top and bottom
//! assembly drawings (#2486) on pages of their own. Same embedded fonts and
//! content-stream surface as the schematic PDF. Rules:
//! `docs/BOARD_OUTPUT.md`, "Print" and "Assembly drawings".

mod assembly;
mod draw;
mod frame;

use pdf_writer::{Finish, Name, Pdf, Rect, Ref};
use serde::{Deserialize, Serialize};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{BoardSide, PcbLayer};
use signex_types::schematic::SheetOrientation;
use thiserror::Error;

use super::embed::{FontUsage, write_fonts};
use super::surface::PdfSurface;
use super::{MM_TO_PT, PageSize, PdfError};
use crate::assembly::{assembly_drawing, assembly_ink};

pub(crate) use frame::PageFrame;

/// A group of board layers printed together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardLayerSet {
    /// Top copper and every top-side technical layer (overlay, mask,
    /// paste, assembly, courtyard).
    Top,
    /// The same for the bottom side.
    Bottom,
    /// The inner copper layers.
    Inner,
    /// The outline, mechanical and documentation layers.
    Mechanical,
    /// The top assembly drawing (#2486): the top Fab, Assembly, Designator
    /// and Fiducial layers, each part's designator, the outline and the
    /// Dimensions layer. Always a page of its own.
    AssemblyTop,
    /// The bottom assembly drawing, the same from the bottom side.
    AssemblyBottom,
}

impl BoardLayerSet {
    /// Every set, in the order the dialog lists them and pages print.
    pub const ALL: [Self; 6] = [
        Self::Top,
        Self::Bottom,
        Self::Inner,
        Self::Mechanical,
        Self::AssemblyTop,
        Self::AssemblyBottom,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Inner => "inner",
            Self::Mechanical => "mechanical",
            Self::AssemblyTop => "assembly_top",
            Self::AssemblyBottom => "assembly_bottom",
        }
    }

    /// The side of an assembly drawing set; `None` for a layer set.
    pub const fn assembly_side(self) -> Option<BoardSide> {
        match self {
            Self::AssemblyTop => Some(BoardSide::Top),
            Self::AssemblyBottom => Some(BoardSide::Bottom),
            Self::Top | Self::Bottom | Self::Inner | Self::Mechanical => None,
        }
    }

    /// Whether a page of this set alone is the one the bottom side mirrors.
    const fn is_bottom(self) -> bool {
        matches!(self, Self::Bottom | Self::AssemblyBottom)
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|set| set.id() == id.trim())
    }

    /// Whether `layer` prints with this set.
    pub fn contains(self, layer: PcbLayer) -> bool {
        match self {
            Self::Inner => matches!(
                layer,
                PcbLayer::Copper(signex_types::pcb_layer::CopperLayer::Inner(_))
            ),
            Self::Mechanical => match layer {
                PcbLayer::Mechanical(_) => true,
                PcbLayer::Technical(t) => t.side().is_none(),
                PcbLayer::Copper(_) => false,
            },
            Self::Top => layer.side() == Some(BoardSide::Top),
            Self::Bottom => layer.side() == Some(BoardSide::Bottom),
            Self::AssemblyTop => assembly_ink(BoardSide::Top, layer).is_some(),
            Self::AssemblyBottom => assembly_ink(BoardSide::Bottom, layer).is_some(),
        }
    }
}

/// One page per set, or every set on one page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardPrintPages {
    #[default]
    PerSet,
    Composite,
}

/// The board at its real size, or as large as the page allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardPrintScale {
    #[default]
    Fit,
    OneToOne,
}

/// What a board print shows. The default prints the top and bottom sides
/// and the outline layers on their own pages, fitted, in colour.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardPrintOptions {
    /// The sets to print; printed in [`BoardLayerSet::ALL`] order.
    pub sets: Vec<BoardLayerSet>,
    pub pages: BoardPrintPages,
    /// Draw a page of the bottom set mirrored, as seen from below.
    pub mirror_bottom: bool,
    pub scale: BoardPrintScale,
    /// Every layer in black instead of its colour.
    pub monochrome: bool,
    /// The board outline on every page.
    pub outline: bool,
    /// Pad and via holes drawn open.
    pub drill_marks: bool,
    pub title_block: bool,
    pub page_size: PageSize,
    pub orientation: SheetOrientation,
}

impl Default for BoardPrintOptions {
    fn default() -> Self {
        Self {
            sets: vec![
                BoardLayerSet::Top,
                BoardLayerSet::Bottom,
                BoardLayerSet::Mechanical,
            ],
            pages: BoardPrintPages::PerSet,
            mirror_bottom: true,
            scale: BoardPrintScale::Fit,
            monochrome: false,
            outline: true,
            drill_marks: true,
            title_block: true,
            page_size: PageSize::IsoA4,
            orientation: SheetOrientation::Landscape,
        }
    }
}

impl BoardPrintOptions {
    /// The pages these options print: their sets, and whether each is
    /// mirrored. Empty when no set is chosen. An assembly drawing is always
    /// a page of its own, after a composite page of the layer sets.
    pub fn page_plan(&self) -> Vec<(Vec<BoardLayerSet>, bool)> {
        let sets: Vec<BoardLayerSet> = BoardLayerSet::ALL
            .into_iter()
            .filter(|set| self.sets.contains(set))
            .collect();
        let alone = |set: BoardLayerSet| (vec![set], self.mirror_bottom && set.is_bottom());
        match self.pages {
            BoardPrintPages::Composite => {
                let (drawings, layers): (Vec<_>, Vec<_>) =
                    sets.into_iter().partition(|s| s.assembly_side().is_some());
                let composite = (!layers.is_empty()).then_some((layers, false));
                composite
                    .into_iter()
                    .chain(drawings.into_iter().map(alone))
                    .collect()
            }
            BoardPrintPages::PerSet => sets.into_iter().map(alone).collect(),
        }
    }
}

/// The words a title block prints, in the user's language: the caller's,
/// since this crate has no message catalog.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardPrintText {
    /// The board's name.
    pub title: String,
    /// The name of each set.
    pub sets: Vec<(BoardLayerSet, String)>,
    /// Shown for a page drawn mirrored.
    pub mirrored: String,
}

impl BoardPrintText {
    fn set_name(&self, set: BoardLayerSet) -> &str {
        self.sets
            .iter()
            .find(|(s, _)| *s == set)
            .map_or(set.id(), |(_, name)| name.as_str())
    }
}

/// What one printed page holds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoardPdfPage {
    pub sets: Vec<BoardLayerSet>,
    pub mirrored: bool,
    /// Page mm per board mm: 1 at 1:1.
    pub scale: f64,
    /// The board's extent as printed, in page mm.
    pub printed_size_mm: (f64, f64),
    /// The board runs past the printable area (at 1:1 on a page too
    /// small): part of it is off the page.
    pub overflows: bool,
}

#[derive(Debug, Clone)]
pub struct BoardPdfOutput {
    pub bytes: Vec<u8>,
    pub pages: Vec<BoardPdfPage>,
    /// Characters no bundled font covers, sorted and unique.
    pub uncovered_chars: Vec<char>,
}

#[derive(Debug, Error)]
pub enum BoardPdfError {
    #[error("no layer set is chosen to print")]
    NoLayerSets,
    #[error(transparent)]
    Pdf(#[from] PdfError),
}

/// Draw `board` as `options` ask and return the PDF.
pub fn board_pdf(
    board: &PcbBoard,
    options: &BoardPrintOptions,
    text: &BoardPrintText,
) -> Result<BoardPdfOutput, BoardPdfError> {
    let (page_w_mm, page_h_mm) = options.page_size.dimensions_mm(options.orientation);
    let mut fonts = FontUsage::new();
    let (contents, pages) = render_pages(board, options, text, &mut fonts)?;
    let bytes = assemble(&contents, &fonts, page_w_mm, page_h_mm)?;
    Ok(BoardPdfOutput {
        bytes,
        pages,
        uncovered_chars: fonts.uncovered_chars(),
    })
}

/// Each page's content stream and what it holds, the fonts they use
/// gathered in `fonts`.
fn render_pages(
    board: &PcbBoard,
    options: &BoardPrintOptions,
    text: &BoardPrintText,
    fonts: &mut FontUsage,
) -> Result<(Vec<Vec<u8>>, Vec<BoardPdfPage>), BoardPdfError> {
    let plan = options.page_plan();
    if plan.is_empty() {
        return Err(BoardPdfError::NoLayerSets);
    }
    // Two refs per page plus the fonts must fit an i32 ref id.
    if plan.len() > (i32::MAX as usize) / 4 {
        return Err(PdfError::TooManyPages(plan.len()).into());
    }
    let page_mm = options.page_size.dimensions_mm(options.orientation);
    let mut contents = Vec::with_capacity(plan.len());
    let mut pages = Vec::with_capacity(plan.len());
    let fills = pcb_geom::zone_fills(board);
    for (index, (sets, mirrored)) in plan.iter().enumerate() {
        let mut surface = PdfSurface::new(fonts);
        let drawing = match sets.as_slice() {
            [set] => set
                .assembly_side()
                .map(|side| assembly_drawing(board, side)),
            _ => None,
        };
        let frame = match &drawing {
            Some(drawing) => {
                let frame =
                    PageFrame::for_extent((drawing.min, drawing.max), options, page_mm, *mirrored);
                assembly::page(&mut surface, drawing, &frame, options.monochrome);
                frame
            }
            None => {
                let frame = PageFrame::new(board, options, page_mm, *mirrored);
                draw::page(&mut surface, board, &fills, options, &frame, sets);
                frame
            }
        };
        if options.title_block {
            let label = sets
                .iter()
                .map(|s| text.set_name(*s))
                .collect::<Vec<_>>()
                .join(" + ");
            let label = if *mirrored {
                format!("{label} ({})", text.mirrored)
            } else {
                label
            };
            let block = draw::TitleBlock {
                title: &text.title,
                label: &label,
                scale: frame.scale(),
                page: (index + 1, plan.len()),
            };
            draw::title_block(&mut surface, &frame, &block, options.monochrome);
        }
        contents.push(surface.finish());
        pages.push(BoardPdfPage {
            sets: sets.clone(),
            mirrored: *mirrored,
            scale: frame.scale(),
            printed_size_mm: frame.printed_size_mm(),
            overflows: frame.overflows(),
        });
    }
    Ok((contents, pages))
}

/// The document: catalog, page tree, one page and content stream per
/// page, then the fonts the pages use.
fn assemble(
    contents: &[Vec<u8>],
    fonts: &FontUsage,
    page_w_mm: f64,
    page_h_mm: f64,
) -> Result<Vec<u8>, PdfError> {
    let page_w = (page_w_mm * MM_TO_PT) as f32;
    let page_h = (page_h_mm * MM_TO_PT) as f32;
    let n = contents.len() as i32;
    let (catalog_id, tree_id) = (Ref::new(1), Ref::new(2));
    let page_refs: Vec<Ref> = (0..n).map(|i| Ref::new(3 + i)).collect();
    let content_refs: Vec<Ref> = (0..n).map(|i| Ref::new(3 + n + i)).collect();
    let font_base = 3 + 2 * n;

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id).kids(page_refs.iter().copied()).count(n);
    let embedded = write_fonts(&mut pdf, fonts, font_base)?;
    for (i, content) in contents.iter().enumerate() {
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

#[cfg(test)]
mod assembly_tests;
#[cfg(test)]
mod tests;
