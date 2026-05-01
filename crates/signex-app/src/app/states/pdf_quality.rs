/// Output PDF resolution preset — Altium parity. Drives the Quality
/// dropdown in the Settings tab. The export pipeline is vector-only
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
}

impl std::fmt::Display for PdfQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PdfQuality::Draft72 => "Draft (72 dpi)",
            PdfQuality::Medium300 => "Medium (300 dpi)",
            PdfQuality::High600 => "High (600 dpi)",
        };
        f.write_str(s)
    }
}
