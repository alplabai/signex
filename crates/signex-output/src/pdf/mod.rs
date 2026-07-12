//! PDF export via `pdf-writer`.
//!
//! See `OUTPUT_PLAN.md` §3. `PdfSurface` (in `surface.rs`) acts as a second
//! render target for the schematic — wires, symbols, labels, title block.
//! Screen (Iced Canvas) and PDF share one source of truth for page layout.
//!
//! ## Font Strategy (v0.8)
//!
//! Roboto + Iosevka TTFs are embedded at compile time (see `font.rs`) but
//! NOT yet emitted as Type0 composite fonts in the PDF — that's a v0.9 job.
//! For v0.8 every text operator references one of four aliases `/F1`–`/F4`
//! pointing at the PDF standard-14 Type1 fonts (Helvetica variants for
//! Roboto, Courier variants for Iosevka). Those standard fonts ship with
//! every PDF reader by spec, so exported PDFs always render text correctly
//! even though the glyphs come from Helvetica/Courier rather than the
//! bundled TTFs.
//!
//! TODO(v0.9): Emit Type0 CIDFontType2 dicts with `/FontFile2` streams
//! pointing at the embedded TTF bytes so the exported PDFs render in the
//! intended Roboto/Iosevka typeface.

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
mod font;
pub(crate) mod layout;
mod page;
pub mod palette;
mod surface;

pub use palette::SchematicPalette;

use crate::svg::{
    SvgElement, SvgEvaluatorInputs, SvgPathCommand, SvgRenderContext, SvgTextAlign, SvgTextVAlign,
};
use colour::ColourMap;
use font::{PdfFont, best_alias_for_text, sanitize_pdf_text, text_advance_pt};
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
        let expr_tables = build_expression_tables(&ctx.sheets);
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

        // Reserve one font ref per PdfFont variant after the content stream
        // refs. Allocated up front so page resources can point at them.
        let font_base: i32 = 3 + 2 * sheet_indices.len() as i32;
        let font_refs: Vec<(font::PdfFont, Ref)> = font::PdfFont::ALL
            .iter()
            .enumerate()
            .map(|(i, &f)| (f, Ref::new(font_base + i as i32)))
            .collect();

        // Build bookmark items up front so the catalog can decide
        // whether to write `/Outlines` and how many `Ref` slots to
        // reserve for outline-item dicts.
        let pending_bookmarks =
            bookmarks::build_bookmarks(ctx, opts, &sheet_indices, page_w_mm, page_h_mm, page_h_pt);
        let bookmarks_active = !pending_bookmarks.is_empty();
        let outline_root_id = Ref::new(font_base + font_refs.len() as i32);
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

        // Emit a minimal Type1 font dict for each bundled font, using the
        // PDF standard-14 name as the BaseFont. Every reader ships these,
        // so text always renders even though we're not (yet) embedding the
        // TTF bytes as a Type0 composite font.
        for (font, font_ref) in &font_refs {
            pdf.type1_font(*font_ref)
                .base_font(Name(font.standard_ps_name().as_bytes()));
        }

        // Build each page with content.
        for (idx, &sheet_idx) in sheet_indices.iter().enumerate() {
            let sheet = &ctx.sheets[sheet_idx];
            let content_ref = content_refs[idx];

            // Emit content stream for this page.
            let content_bytes =
                build_page_content(sheet, opts, ctx, page_w_pt, page_h_pt, &expr_tables)?;

            pdf.stream(content_ref, &content_bytes);

            // Create the page object referencing the content stream.
            let mut page = pdf.page(page_refs[idx]);
            page.parent(page_tree_id)
                .media_box(Rect::new(0.0, 0.0, page_w_pt, page_h_pt))
                .contents(content_ref);

            // /Font resources dict — maps the F1-F4 aliases used in the
            // content streams to the font objects emitted above.
            let mut resources = page.resources();
            let mut fonts = resources.fonts();
            for (font, font_ref) in &font_refs {
                fonts.pair(Name(font.alias().as_bytes()), *font_ref);
            }
            fonts.finish();
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
        })
    }
}


mod content;
#[cfg(test)]
mod tests;

use content::{build_page_content, resolve_page_range};
