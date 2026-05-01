/// Tabs inside the unified Export PDF modal — Preview is the
/// rasterised page view, Settings is the multi-section configuration
/// panel (file picker, additional settings, structure settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfPreviewTab {
    Preview,
    Settings,
}
