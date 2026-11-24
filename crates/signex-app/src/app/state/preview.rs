//! The Export PDF / print preview modal's state; the BOM dialog's is in
//! `bom_preview`.

use std::path::PathBuf;

use super::PageKey;
use super::pdf_dialog::PdfDialogState;
use super::pdf_page_setup::PageSetupInputs;
use super::pdf_preview_raster::{PreviewSlot, RasterSource};

/// Output PDF resolution preset. Drives the Quality dropdown in the
/// Export PDF dialog's Output section. The export pipeline is vector-only
/// today, so DPI only affects the *preview* rasterisation: a higher
/// preset gives a sharper preview when you zoom in. The mapped DPI
/// for the export-side `PdfOptions.dpi` is the picker label (72/300/
/// 600) so future raster fallbacks (embedded images) get the user
/// intent verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfQuality {
    Draft72,
    Medium300,
    High600,
}

impl PdfQuality {
    /// DPI used to rasterise the on-screen preview. Capped well below
    /// the export label so an A4 page doesn't blow up to ~35 MB of
    /// RGBA at 600 DPI.
    pub fn preview_dpi(self) -> f64 {
        match self {
            PdfQuality::Draft72 => 72.0,
            PdfQuality::Medium300 => 144.0,
            PdfQuality::High600 => 200.0,
        }
    }

    /// DPI written to `PdfOptions.dpi` at export time. Vector content
    /// ignores this; future raster fallbacks (embedded images,
    /// rasterised symbol bodies) honour the verbatim picker label.
    pub fn export_dpi(self) -> f32 {
        match self {
            PdfQuality::Draft72 => 72.0,
            PdfQuality::Medium300 => 300.0,
            PdfQuality::High600 => 600.0,
        }
    }

    /// The name the quality pick list shows, rendered at view time.
    pub fn label(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            PdfQuality::Draft72 => ids::PDF_QUALITY_DRAFT,
            PdfQuality::Medium300 => ids::PDF_QUALITY_MEDIUM,
            PdfQuality::High600 => ids::PDF_QUALITY_HIGH,
        }
    }
}

/// Open-print-preview state — rasterised pages + which one is currently
/// shown full-size. Pages are produced by `signex_output::PreviewRasterizer`
/// when the user invokes File → Print Preview (Ctrl+P).
///
/// **Single source of truth.** Every option that's also on
/// `signex_output::PdfOptions` lives ONLY on `pdf_options`; the
/// dispatcher mutates that struct directly so the rasterizer and
/// exporter see one consistent view. Fields on this struct itself are
/// the leftovers — UI presentation (folded sections, quality enum, the
/// Watermark and PDF Layers switches), the rasterised pages, and pan/zoom
/// interaction state.
pub struct PreviewState {
    /// The document the preview was built for; Export writes this one even if
    /// the user has switched tabs since (#852).
    pub source: PathBuf,
    /// The previewed pages in print order. Their pixels are rasterised
    /// lazily: the shown page first, any other when it is navigated to.
    pub pages: Vec<PreviewSlot>,
    /// What the newest rasterisation renders from, so a page navigated to
    /// later is rasterised from the same snapshot.
    pub raster_source: Option<RasterSource>,
    pub selected: usize,
    pub pdf_options: signex_output::PdfOptions,
    pub specific_page_input: String,
    /// Why the preview shows no page when the page range, not the file
    /// picker, selects nothing (#1081): a sheet number no printed page
    /// carries. `None` otherwise.
    pub range_note: Option<String>,
    /// Multiplicative zoom for the preview image. 1.0 = fit-to-viewport;
    /// scroll wheel multiplies by `1.10`/`1/1.10`. Clamped to
    /// `[Self::ZOOM_MIN, Self::ZOOM_MAX]` in the handler so very fast
    /// wheel bursts can't blow the image up to gigabytes.
    pub zoom: f32,
    /// Folded settings sections and the Watermark / PDF Layers switches.
    pub dialog: PdfDialogState,
    /// Pan offset in logical pixels — added to the image origin so the
    /// user can drag a zoomed-in page around the viewport. Reset to
    /// (0, 0) when zoom ≤ 1 (no pan needed) and on page change.
    pub pan: (f32, f32),
    /// In-flight pan drag — `Some((origin_pan, press_x, press_y))`
    /// while the user is holding the mouse down on the preview
    /// surface. Updated every move via the global mouse handler.
    pub panning: Option<((f32, f32), f32, f32)>,
    /// Every sheet this export could cover, as `(path, display name)` in
    /// page order — a snapshot of the `ExportContext` sheet set taken when
    /// the modal opened. The Settings-tab file picker renders from this
    /// rather than re-deriving from the project, so the checkbox list is
    /// guaranteed to match the pages the exporter will emit (a loose
    /// schematic lists itself; a project lists its sheets).
    pub sheet_files: Vec<(PageKey, String)>,
    /// Files the user has ticked for export, a subset of `sheet_files`.
    /// Seeded to all of them on open. Empty = nothing selected, which the
    /// export path rejects with a user-visible error.
    pub selected_files: std::collections::HashSet<PageKey>,
    /// Available variants for the active project — drives the variant
    /// picker dropdown options. The currently-selected value lives on
    /// `pdf_options.variant`.
    pub variants: Vec<String>,
    /// Quality preset shown in the Settings tab dropdown. Mapped to
    /// `pdf_options.dpi` at export time; the preview always rasterises
    /// at `PdfQuality::preview_dpi` (72/144/200) for speed.
    pub quality: PdfQuality,
    /// Typed text of the page-setup fields (custom size, margins, percent)
    /// plus the reason the last edit was rejected or clamped.
    pub page_setup: PageSetupInputs,
    /// Generation of the newest background rasterisation request; results
    /// tagged with an older one are stale and discarded.
    pub raster_generation: u64,
    /// The document the preview was opened for, recorded at open. The
    /// remembered page setup is filed under this, not whichever tab is
    /// active when the preview closes.
    pub source_path: Option<PathBuf>,
    /// Outcome of the last PDF export started from this window, shown in its
    /// footer: where the file went, or why it was not written (#1766).
    pub export_status: Option<String>,
}

impl PreviewState {
    pub const ZOOM_MIN: f32 = 0.25;
    pub const ZOOM_MAX: f32 = 6.0;
    pub const ZOOM_STEP: f32 = 1.10;
}
