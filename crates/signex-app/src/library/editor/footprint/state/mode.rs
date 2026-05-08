//! Editor-mode enum + active-bar menu enum + pad-stack tab strip.

/// Footprint editor mode — gate sketch tooling on / off without
/// rewriting the canvas state machine. Phase 5.3 of the v0.13 sketch-
/// mode plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// Direct pad-list editing (the existing Phase 0–v0.10 surface).
    #[default]
    Normal,
    /// Parametric sketch mode — Phase 6 UI lives behind this.
    Sketch,
    /// 3D body preview (existing v0.10 viewer).
    View3d,
}

/// v0.13 — Altium-style footprint active bar dropdown menus. One per
/// chevron-bearing button in `pads_active_bar`. The dropdown overlay
/// reads this enum to render the matching menu body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpActiveBarMenu {
    /// Selection Filter — 10 footprint-kind toggle pills.
    Filter,
    /// Snap Options — Grids/Guides/Axes pills + 12 snap-object pills.
    Snap,
    /// Place / Move / Drag — gesture menu.
    Place,
    /// Selection — selection-mode picker.
    Select,
    /// Align — full Altium align/distribute menu.
    Align,
    /// 3D Body — 3D Body, Extruded 3D Body.
    Body3d,
    /// Text — String, Text Frame.
    Text,
    /// Shapes — Sketch-mode only.
    Shapes,
}

/// v0.20 — Pad Stack section's tab strip. Matches Altium's three tabs
/// verbatim:
/// - `Simple`: one row per stack family (COPPER / HOLE / PASTE / SOLDER).
/// - `TopMiddleBottom`: COPPER splits into Top / Middle / Bottom rows.
/// - `FullStack`: enumerates the pad's `layers` list verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadStackTab {
    #[default]
    Simple,
    TopMiddleBottom,
    FullStack,
}

impl PadStackTab {
    pub const ALL: &'static [PadStackTab] = &[
        PadStackTab::Simple,
        PadStackTab::TopMiddleBottom,
        PadStackTab::FullStack,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PadStackTab::Simple => "Simple",
            PadStackTab::TopMiddleBottom => "Top-Middle-Bottom",
            PadStackTab::FullStack => "Full Stack",
        }
    }
}
