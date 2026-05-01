/// Open-print-preview state — rasterised pages + which one is currently
/// shown full-size. Pages are produced by `signex_output::PreviewRasterizer`
/// when the user invokes File -> Print Preview (Ctrl+P).
///
/// **Single source of truth.** Every option that's also on
/// `signex_output::PdfOptions` lives ONLY on `pdf_options`; the
/// dispatcher mutates that struct directly so the rasterizer and
/// exporter see one consistent view. Fields on this struct itself are
/// the leftovers — UI presentation (active tab, quality enum), the
/// rasterised pages, and pan/zoom interaction state.
pub struct PreviewState {
    pub pages: Vec<signex_output::PreviewPage>,
    pub page_handles: Vec<iced::widget::image::Handle>,
    pub selected: usize,
    pub pdf_options: signex_output::PdfOptions,
    pub specific_page_input: String,
    /// Multiplicative zoom for the preview image. 1.0 = fit-to-viewport;
    /// scroll wheel multiplies by `1.10`/`1/1.10`. Clamped to
    /// `[Self::ZOOM_MIN, Self::ZOOM_MAX]` in the handler so very fast
    /// wheel bursts can't blow the image up to gigabytes.
    pub zoom: f32,
    /// Currently-shown tab inside the Export PDF modal.
    pub active_tab: super::PdfPreviewTab,
    /// Pan offset in logical pixels — added to the image origin so the
    /// user can drag a zoomed-in page around the viewport. Reset to
    /// (0, 0) when zoom <= 1 (no pan needed) and on page change.
    pub pan: (f32, f32),
    /// In-flight pan drag — `Some((origin_pan, press_x, press_y))`
    /// while the user is holding the mouse down on the preview
    /// surface. Updated every move via the global mouse handler.
    pub panning: Option<((f32, f32), f32, f32)>,
    /// Files chosen for export from the active project's sheet list.
    /// Empty = all files (default at open). When non-empty, only the
    /// listed paths are rasterised + exported. Driven by the file
    /// picker in the Settings tab.
    pub selected_files: std::collections::HashSet<std::path::PathBuf>,
    /// Available variants for the active project — drives the variant
    /// picker dropdown options. The currently-selected value lives on
    /// `pdf_options.variant`.
    pub variants: Vec<String>,
    /// Quality preset shown in the Settings tab dropdown. Mapped to
    /// `pdf_options.dpi` at export time; the preview always rasterises
    /// at 96 DPI for speed.
    pub quality: super::PdfQuality,
}

impl PreviewState {
    pub const ZOOM_MIN: f32 = 0.25;
    pub const ZOOM_MAX: f32 = 6.0;
    pub const ZOOM_STEP: f32 = 1.10;
}
