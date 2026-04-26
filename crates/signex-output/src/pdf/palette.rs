//! Schematic colour palette for the PDF / preview pipeline.
//!
//! The on-screen schematic is themed via `signex_types::CanvasColors`.
//! For the exported PDF and the preview rasteriser we lift those
//! values into f32 RGB triples — that's what `SvgRenderContext`
//! consumes for stroke / fill state. Mapping happens once when the
//! Print Preview modal opens (or when an export is triggered) so
//! every wire / symbol / label in the resulting PDF matches what the
//! user is looking at on the canvas.
//!
//! Pre-existing tests + the empty `PdfOptions::default()` keep using
//! the historical eeschema-style palette (`SchematicPalette::classic()`).
//! The unified Print Preview hands the active theme's palette in
//! when it kicks off an export, so users see Altium-style cream /
//! Catppuccin Mocha / etc. honoured on paper too.

use signex_types::theme::CanvasColors;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchematicPalette {
    pub paper: (f32, f32, f32),
    pub wire: (f32, f32, f32),
    pub bus: (f32, f32, f32),
    pub bus_entry: (f32, f32, f32),
    pub junction: (f32, f32, f32),
    pub no_connect: (f32, f32, f32),
    pub symbol_stroke: (f32, f32, f32),
    pub symbol_fill: (f32, f32, f32),
    pub pin: (f32, f32, f32),
    pub reference: (f32, f32, f32),
    pub value: (f32, f32, f32),
    pub net_label: (f32, f32, f32),
    pub global_label: (f32, f32, f32),
    pub hier_label: (f32, f32, f32),
    pub power_label: (f32, f32, f32),
    /// Sheet boundary stroke. Render parity sets this to a faint
    /// shade of the paper colour; PDF parity wants something that
    /// stays visible on dark themes too.
    pub sheet_border: (f32, f32, f32),
    /// Free-floating text annotations ("Notes" in Altium parlance).
    pub note_text: (f32, f32, f32),
    /// Hierarchical child-sheet boundary + name/filename text.
    pub child_sheet_stroke: (f32, f32, f32),
    pub child_sheet_text: (f32, f32, f32),
    /// Generic schematic body-text colour — used for ref/value/pin
    /// labels, drawing strokes, and other "black ink" fields when
    /// no more-specific palette entry applies. Matches what the
    /// canvas paints in pin-name and ref-text colours.
    pub field_text: (f32, f32, f32),
}

impl SchematicPalette {
    /// Historical eeschema-style palette — cream paper, dark-blue
    /// wires, mustard symbol bodies. Preserved for tests and as the
    /// default-for-tests `PdfOptions::default()` palette so the
    /// existing /Page bytes don't shift under tests.
    pub const fn classic() -> Self {
        Self {
            paper: (1.0, 1.0, 1.0),
            wire: (0.09, 0.21, 0.66),
            bus: (0.1, 0.2, 0.56),
            bus_entry: (0.12, 0.24, 0.62),
            junction: (0.03, 0.56, 0.2),
            no_connect: (0.78, 0.18, 0.18),
            symbol_stroke: (0.53, 0.41, 0.04),
            symbol_fill: (0.93, 0.93, 0.56),
            pin: (0.53, 0.41, 0.04),
            reference: (0.53, 0.41, 0.04),
            value: (0.0, 0.4, 0.4),
            net_label: (0.08, 0.08, 0.08),
            global_label: (0.14, 0.24, 0.52),
            hier_label: (0.28, 0.2, 0.06),
            power_label: (0.42, 0.09, 0.09),
            sheet_border: (0.78, 0.78, 0.78),
            note_text: (0.14, 0.14, 0.14),
            child_sheet_stroke: (0.25, 0.25, 0.25),
            child_sheet_text: (0.18, 0.18, 0.18),
            field_text: (0.1, 0.1, 0.1),
        }
    }
}

impl Default for SchematicPalette {
    fn default() -> Self {
        Self::classic()
    }
}

impl From<&CanvasColors> for SchematicPalette {
    fn from(c: &CanvasColors) -> Self {
        Self {
            paper: rgb(c.paper),
            wire: rgb(c.wire),
            bus: rgb(c.bus),
            bus_entry: rgb(c.bus),
            junction: rgb(c.junction),
            no_connect: rgb(c.no_connect),
            symbol_stroke: rgb(c.body),
            symbol_fill: rgb(c.body_fill),
            pin: rgb(c.pin),
            reference: rgb(c.reference),
            value: rgb(c.value),
            net_label: rgb(c.net_label),
            global_label: rgb(c.global_label),
            hier_label: rgb(c.hier_label),
            power_label: rgb(c.power),
            // Sheet border is intentionally derived from the
            // selection accent so it's visible on both light and
            // dark canvases without bleeding into the paper.
            sheet_border: rgb(c.selection),
            // Notes / free-text re-use the canvas reference colour
            // (typically the most legible "ink" tone over paper).
            note_text: rgb(c.reference),
            // Child-sheet hierarchy chrome uses the body stroke so
            // it sits visually with the symbols — Altium does the
            // same on its Smart PDF output.
            child_sheet_stroke: rgb(c.body),
            child_sheet_text: rgb(c.reference),
            field_text: rgb(c.reference),
        }
    }
}

impl From<CanvasColors> for SchematicPalette {
    fn from(c: CanvasColors) -> Self {
        Self::from(&c)
    }
}

/// `signex_types::theme::Color` is u8 RGBA — strip alpha and divide
/// by 255 so the renderer can feed PDF / tiny-skia f32 colour ops.
fn rgb(c: signex_types::theme::Color) -> (f32, f32, f32) {
    (
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, canvas_colors};

    #[test]
    fn classic_palette_matches_legacy_constants() {
        let p = SchematicPalette::classic();
        assert!((p.wire.0 - 0.09).abs() < 1e-5);
        assert!((p.symbol_stroke.0 - 0.53).abs() < 1e-5);
        assert!((p.junction.1 - 0.56).abs() < 1e-5);
    }

    #[test]
    fn from_canvas_colors_normalises_u8_rgb() {
        let signex = canvas_colors(ThemeId::Signex);
        let pal = SchematicPalette::from(&signex);
        assert!(pal.paper.0 >= 0.0 && pal.paper.0 <= 1.0);
        assert!(pal.wire.2 >= 0.0 && pal.wire.2 <= 1.0);
    }

    #[test]
    fn altium_dark_paper_is_dark() {
        // Catppuccin Mocha is dark; the canvas paper should be a
        // dark colour (channels close to 0), not white.
        let mocha = canvas_colors(ThemeId::CatppuccinMocha);
        let pal = SchematicPalette::from(&mocha);
        assert!(pal.paper.0 < 0.5);
        assert!(pal.paper.1 < 0.5);
        assert!(pal.paper.2 < 0.5);
    }
}
