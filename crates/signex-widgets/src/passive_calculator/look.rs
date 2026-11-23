//! The colours and faces the passive calculator draws with. This crate
//! cannot resolve a theme's optional chrome tokens the way the app does, so
//! the caller fills [`CalculatorPalette`] from its resolved tokens and hands
//! the mode-switch and enable-section looks over in [`CalculatorLook`].

use iced::{Color, Font};

use crate::enable_section::EnableSectionStyle;
use crate::segmented::SegmentedStyle;

/// One resolved colour per Graphite token the calculator paints with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalculatorPalette {
    /// Values and running text (`ink`).
    pub ink: Color,
    /// Line art of the network drawing (`ink2`).
    pub ink2: Color,
    /// Field labels, the unit suffix and the section label (`ink3`).
    pub ink3: Color,
    /// Captions, metric keys, placeholders and muted deltas (`ink4`).
    pub ink4: Color,
    pub accent: Color,
    /// Accent used as text: the ghost button.
    pub accent_text: Color,
    /// Field, select, card and metric-strip outlines.
    pub border: Color,
    /// Hairlines between metric cells; the drawing box outline.
    pub line_soft: Color,
    /// The idle scrollbar thumb.
    pub line_strong: Color,
    /// The dialog surface; the veil over a switched-off subsection.
    pub surface: Color,
    /// Component cards and the empty-state box.
    pub panel: Color,
    /// Text-field fill.
    pub field: Color,
    /// Recessed fill: the SI-prefix picker inside a field, the drawing box.
    pub sunken: Color,
    pub primary: Color,
    pub primary_ink: Color,
    /// Pointer-over wash of the ghost button and the section header.
    pub hover: Color,
    pub ok: Color,
    pub ok_soft: Color,
    pub warn: Color,
    pub warn_soft: Color,
    pub error: Color,
}

/// Everything the calculator's views take besides the data.
pub struct CalculatorLook<'a> {
    pub palette: CalculatorPalette,
    /// The monospace face of values and codes.
    pub mono: Font,
    /// The mode switch under the dialog header (BB4).
    pub segmented: SegmentedStyle,
    /// The production-date subsection (enable-first, BB3).
    pub enable_section: EnableSectionStyle<'a>,
}
