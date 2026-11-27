//! The Export PDF dialog's Watermark, PDF Layers and Bookmarks controls
//! (#1298), carried under `PrintPreviewMsg::PdfOption`. None of them changes
//! the rasterised page: the preview draws the watermark over it.

#[derive(Debug, Clone)]
pub enum PdfOptionMsg {
    /// "Print a watermark" was ticked or cleared.
    WatermarkToggled(bool),
    /// A watermark preset (DRAFT, CONFIDENTIAL, Custom) was picked.
    WatermarkPresetPicked(crate::app::state::WatermarkPreset),
    /// The watermark Text field was edited.
    WatermarkTextChanged(String),
    /// The watermark opacity field or slider changed (percent, as text).
    WatermarkOpacityChanged(String),
    /// The watermark angle field or the 0 / 45 / 90 switch changed
    /// (degrees, as text).
    WatermarkAngleChanged(String),
    /// The watermark Colour field was edited (#2609, `#RRGGBB` as text).
    WatermarkColourChanged(String),
    /// A watermark colour swatch was picked (#2609).
    WatermarkColourPicked([u8; 3]),
    /// All pages or the first only was picked for the watermark.
    WatermarkPagesPicked(signex_output::WatermarkPages),
    /// "Add date and revision under the text" was ticked or cleared.
    WatermarkRevisionToggled(bool),
    /// "Write PDF layers" was ticked or cleared.
    PdfLayersToggled(bool),
    /// A PDF layer's "visible when the PDF opens" box was ticked or cleared.
    PdfLayerVisibilityToggled(signex_output::PdfLayer, bool),
    /// "One bookmark per sheet" was ticked or cleared.
    BookmarkSheetsToggled(bool),
    /// "One bookmark per part" was ticked or cleared.
    BookmarkPartsToggled(bool),
    /// The part bookmarks' grouping (by sheet, A–Z) was picked.
    PartBookmarkOrderPicked(signex_output::PartBookmarkOrder),
}

impl From<PdfOptionMsg> for super::PrintPreviewMsg {
    fn from(msg: PdfOptionMsg) -> Self {
        Self::PdfOption(msg)
    }
}

impl From<PdfOptionMsg> for super::Message {
    fn from(msg: PdfOptionMsg) -> Self {
        Self::PrintPreview(msg.into())
    }
}
