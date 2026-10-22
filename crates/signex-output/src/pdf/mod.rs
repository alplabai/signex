//! PDF export via `pdf-writer`.
//!
//! See `OUTPUT_PLAN.md` §3. `PdfSurface` (in `surface.rs`) acts as a second
//! render target for the schematic — wires, symbols, labels, title block.
//! Screen (Iced Canvas) and PDF share one source of truth for page layout.
//!
//! ## Fonts
//!
//! Roboto + Iosevka TTFs (see `font.rs`) are embedded as Type0 composite fonts
//! (`embed.rs`): `Identity-H` encoding, a `CIDFontType2` descendant with a
//! `/W` array, and a `/FontFile2` stream subset (`subset.rs`) to the glyphs
//! the document actually shows, plus a `/ToUnicode` CMap so the text stays
//! selectable and searchable. Content streams address fonts through the
//! aliases `/F1`-`/F4` and show text as 2-byte glyph ids.

use pdf_writer::{Finish, Name, Pdf, Rect, Ref};
use signex_types::markup::{
    ExpressionEvalContext, RichSegment, evaluate_expressions, parse_signex_markup,
};
use thiserror::Error;

use crate::expression::{ExpressionTables, build_expression_tables, sheet_cell_value};
use crate::template::TemplateId;
use crate::{ExportContext, Exporter, SubstitutionContext};

mod bookmarks;
mod colour;
mod embed;
mod font;
pub(crate) mod layout;
mod page;
mod page_setup;
pub mod palette;
mod subset;
mod surface;

pub use page_setup::{
    MAX_PAGE_EDGE_MM, MAX_SCALE_PERCENT, MIN_CUSTOM_EDGE_MM, MIN_PRINTABLE_MM, MarginEdge,
    MarginUpdate, PageSetupError, PageSizeKind, PdfScaleMode, parse_custom_edge_mm, parse_mm,
    parse_percent,
};
pub use palette::SchematicPalette;

use crate::svg::{
    SvgElement, SvgEvaluatorInputs, SvgPathCommand, SvgRenderContext, SvgTextAlign, SvgTextVAlign,
};
use colour::ColourMap;
use embed::{FontUsage, REFS_PER_FONT, write_fonts};
use font::{PdfFont, best_alias_for_text, normalize_pdf_text, text_advance_pt};
use surface::PdfSurface;

/// 1 mm in PDF points (1 pt = 1/72 inch).
const MM_TO_PT: f64 = 72.0 / 25.4;

pub struct PdfExporter;

#[derive(Debug, Clone)]
pub struct PdfOptions {
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub colour_mode: ColourMode,
    pub page_range: PageRange,
    pub sheet_template: Option<TemplateId>,
    pub margins: Margins,
    pub scale: PdfScale,
    pub include_title_block: bool,
    /// Render PCB content (when present) in the chosen colour mode.
    /// The schematic-only PDF path ignores this, but it's stored on the
    /// options so the future PCB exporter and the unified Print Preview
    /// modal share one source of truth.
    pub pcb_colour_mode: ColourMode,
    /// DPI hint for raster fallbacks and PDF object resolution. Vector
    /// content doesn't depend on DPI, but rasterized previews and any
    /// embedded image content do; the value seeds `PreviewOptions.dpi`
    /// when set up by the unified preview modal.
    pub dpi: f32,
    /// Variant override for sheet rendering — `None` means use the
    /// project's active variant (or Base when none is set). The
    /// preview/export modal can pin this to a specific variant
    /// without mutating the project's active variant.
    pub variant: Option<String>,
    /// Use Altium-style "physical structure" expansion: logical
    /// sheets expand to physical sheets named after the variant.
    /// Today it controls the title block's `${VARIANT}` token and
    /// the bookmark sheet titles; full per-instance sheet rewriting
    /// lands once signex-types models per-variant component data.
    pub use_physical_structure: bool,
    /// Per-instance designator/net-label/port rewriting. Today
    /// signex-types stores variants as `Vec<String>` only — there's
    /// no per-variant override map — so these toggles are accepted
    /// and round-tripped but produce no visible difference until
    /// the schema gains per-variant fields. Promote the gating
    /// inside `svg/mod.rs` (designators) and `bookmarks::format_*`
    /// (labels/ports) one-liner-style when that lands.
    pub physical_designators: bool,
    pub physical_net_labels: bool,
    pub physical_ports: bool,
    /// Drops `${SHEETNUMBER}` / `${DOCUMENTNUMBER}` from the title
    /// block when off. Live today via `SubstitutionContext`.
    pub physical_sheet_number: bool,
    pub physical_document_number: bool,
    /// Render schematic chrome elements when set. False hides each
    /// element from the exported PDF. Mirrors Altium's "Schematics
    /// include" checklist verbatim.
    ///
    /// Live toggles (renderer honours the value):
    ///   `include_no_erc_markers`, `include_notes`.
    ///
    /// Dormant toggles — Standard's schema has no equivalent concept,
    /// so these are stored for Altium-import parity but produce no
    /// observable difference today: `include_parameter_sets` (Altium
    /// parameter-set objects), `include_probes` (Altium probe
    /// markers), `include_blankets` (Altium blanket regions),
    /// `include_collapsed_notes` (Altium collapsed-note placards).
    /// Toggle them ahead of time so that round-tripping an Altium
    /// project keeps the user's intent — when the corresponding
    /// signex-types feature lands the gating is one-line.
    pub include_no_erc_markers: bool,
    pub include_parameter_sets: bool,
    pub include_probes: bool,
    pub include_blankets: bool,
    pub include_notes: bool,
    pub include_collapsed_notes: bool,
    /// Bookmark target zoom level for component / net jumps inside the
    /// PDF reader. Range 0.0 (Far) → 1.0 (Close).
    pub bookmark_zoom: f32,
    /// Emit per-net "Generate Nets Information" bookmarks.
    pub generate_nets_info: bool,
    /// Sub-bookmarks for nets — each toggle scopes which entity
    /// children appear under the per-net bookmark.
    pub bookmark_pins: bool,
    pub bookmark_net_labels: bool,
    pub bookmark_ports: bool,
    /// Add component-parameter rows to bookmarks for components.
    pub include_component_parameters: bool,
    /// Emit a top-level "Components & Nets" pair of bookmarks instead
    /// of nesting them under the sheet they appear on.
    pub global_bookmarks: bool,
    /// Schematic-element colour palette. The unified Print Preview
    /// passes the active theme's `CanvasColors` here so PDF wires /
    /// symbols / labels match the on-screen schematic. Default is
    /// the legacy `SchematicPalette::classic()` (cream paper / dark-
    /// blue wires) so existing direct-export callers keep their
    /// historical look.
    pub palette: SchematicPalette,
}

#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub page_count: usize,
    /// Characters none of the bundled fonts cover. They are drawn as the
    /// font's `.notdef` box; the caller should warn the user. Sorted, unique.
    pub uncovered_chars: Vec<char>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageSize {
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
    Custom { width_mm: f64, height_mm: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourMode {
    Colour,
    Grayscale,
    BlackAndWhite,
}

#[derive(Debug, Clone)]
pub enum PageRange {
    All,
    Current,
    Specific(Vec<usize>),
    Range(usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top_mm: f64,
    pub right_mm: f64,
    pub bottom_mm: f64,
    pub left_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfScale {
    FitToPage,
    OneToOne,
    Percent(f64),
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSize::IsoA4,
            orientation: Orientation::Landscape,
            colour_mode: ColourMode::Colour,
            page_range: PageRange::All,
            sheet_template: None,
            margins: Margins {
                top_mm: 10.0,
                right_mm: 10.0,
                bottom_mm: 10.0,
                left_mm: 10.0,
            },
            scale: PdfScale::FitToPage,
            include_title_block: true,
            pcb_colour_mode: ColourMode::Colour,
            dpi: 96.0,
            variant: None,
            use_physical_structure: true,
            physical_designators: true,
            physical_net_labels: true,
            physical_ports: true,
            physical_sheet_number: true,
            physical_document_number: true,
            include_no_erc_markers: true,
            include_parameter_sets: true,
            include_probes: true,
            include_blankets: true,
            include_notes: true,
            include_collapsed_notes: false,
            bookmark_zoom: 0.5,
            generate_nets_info: true,
            bookmark_pins: true,
            bookmark_net_labels: true,
            bookmark_ports: true,
            include_component_parameters: true,
            global_bookmarks: false,
            palette: SchematicPalette::classic(),
        }
    }
}

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("no sheets in export context")]
    NoSheets,

    #[error("page range references sheet {0} but project only has {1} sheets")]
    OutOfRange(usize, usize),

    #[error("font subsetting failed: {0}")]
    Font(String),

    /// MD-29: PDF Ref is `i32` so a project with billions of sheets
    /// would silently wrap to a negative id. We bail out before
    /// allocating any refs to keep the output well-formed.
    #[error("project has {0} pages — exceeds the safe PDF Ref id limit")]
    TooManyPages(usize),
}

impl Exporter for PdfExporter {
    type Options = PdfOptions;
    type Output = PdfOutput;
    type Error = PdfError;

    fn export(
        &self,
        ctx: &ExportContext,
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        if ctx.sheets.is_empty() {
            return Err(PdfError::NoSheets);
        }

        let sheet_indices = resolve_page_range(&opts.page_range, ctx.sheets.len())?;
        let expr_tables = build_expression_tables(&ctx.sheets, ctx.netlist.as_ref());
        let (page_w_mm, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let page_w_pt = (page_w_mm * MM_TO_PT) as f32;
        let page_h_pt = (page_h_mm * MM_TO_PT) as f32;

        let mut pdf = Pdf::new();

        let catalog_id = Ref::new(1);
        let page_tree_id = Ref::new(2);

        // MD-29: bound the page count up front so the `usize → i32` cast
        // below cannot overflow into a negative `Ref` id. Each sheet
        // claims one page Ref + one content Ref; reserve headroom for
        // fonts, outline root, and per-bookmark refs by leaving an
        // ample margin under `i32::MAX`.
        let max_sheets = (i32::MAX as usize) / 4;
        if sheet_indices.len() > max_sheets {
            return Err(PdfError::TooManyPages(sheet_indices.len()));
        }

        // Reserve one Ref per page, starting at 3 (after catalog + page tree).
        let page_refs: Vec<Ref> = (0..sheet_indices.len())
            .map(|i| Ref::new(3 + i as i32))
            .collect();

        // Reserve content stream Refs after page Refs.
        let content_refs: Vec<Ref> = (0..sheet_indices.len())
            .map(|i| Ref::new(3 + sheet_indices.len() as i32 + i as i32))
            .collect();

        // Reserve REFS_PER_FONT refs per PdfFont variant after the content
        // stream refs. Only the fonts the document uses are written.
        let font_base: i32 = 3 + 2 * sheet_indices.len() as i32;
        let font_ref_count = font::PdfFont::ALL.len() as i32 * REFS_PER_FONT;

        // Build bookmark items up front so the catalog can decide
        // whether to write `/Outlines` and how many `Ref` slots to
        // reserve for outline-item dicts.
        let pending_bookmarks =
            bookmarks::build_bookmarks(ctx, opts, &sheet_indices, page_w_mm, page_h_mm, page_h_pt);
        let bookmarks_active = !pending_bookmarks.is_empty();
        let outline_root_id = Ref::new(font_base + font_ref_count);
        let bookmark_id_base = outline_root_id.get() + 1;

        let mut catalog = pdf.catalog(catalog_id);
        catalog.pages(page_tree_id);
        if bookmarks_active {
            catalog.outlines(outline_root_id);
        }
        catalog.finish();

        pdf.pages(page_tree_id)
            .kids(page_refs.iter().copied())
            .count(page_refs.len() as i32);

        // Build every page's content first: encoding text is what discovers
        // the glyphs each embedded font must carry.
        let mut fonts = FontUsage::new();
        let contents = sheet_indices
            .iter()
            .map(|&sheet_idx| {
                build_page_content(
                    &ctx.sheets[sheet_idx],
                    opts,
                    ctx,
                    page_w_pt,
                    page_h_pt,
                    &expr_tables,
                    &mut fonts,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let embedded = write_fonts(&mut pdf, &fonts, font_base)?;

        for (idx, content_bytes) in contents.iter().enumerate() {
            let content_ref = content_refs[idx];
            pdf.stream(content_ref, content_bytes);

            // Create the page object referencing the content stream.
            let mut page = pdf.page(page_refs[idx]);
            page.parent(page_tree_id)
                .media_box(Rect::new(0.0, 0.0, page_w_pt, page_h_pt))
                .contents(content_ref);

            // /Font resources dict — maps the F1-F4 aliases used in the
            // content streams to the embedded Type0 font objects.
            let mut resources = page.resources();
            let mut font_dict = resources.fonts();
            for f in &embedded {
                font_dict.pair(Name(f.font.alias().as_bytes()), f.type0);
            }
            font_dict.finish();
            resources.finish();

            page.finish();
        }

        // Outline tree must be written after every page Ref has been
        // allocated so /Dest entries can point at concrete pages.
        if bookmarks_active {
            bookmarks::emit_bookmarks(
                &mut pdf,
                &pending_bookmarks,
                outline_root_id,
                bookmark_id_base,
                &page_refs,
                opts,
            );
        }

        let bytes = pdf.finish();

        Ok(PdfOutput {
            bytes,
            page_count: page_refs.len(),
            uncovered_chars: fonts.uncovered_chars(),
        })
    }
}

mod content;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unicode_tests;

use content::{build_page_content, resolve_page_range};
