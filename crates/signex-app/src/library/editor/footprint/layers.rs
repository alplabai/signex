//! Footprint editor layer registry — the seven Altium-spec layers a
//! footprint surfaces: F.Cu / B.Cu / F.SilkS / B.SilkS / F.Fab / B.Fab
//! / Edge.Cuts (the courtyard polygon stays on Edge.Cuts in this Phase
//! 2 cut; F.CrtYd / B.CrtYd land alongside the proper paste/mask
//! toggles in v0.9.x).

use iced::Color;

/// One of the visible layers shown in the Footprint tab toolbar.
///
/// The Standard/Altium layer model is much wider than this — F/B
/// paste, mask, adhesive, etc. all exist — but the MVP exposes only
/// the layers the user must see to draw a pad + courtyard. Extending
/// this enum is purely additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FpLayer {
    FCu,
    BCu,
    FSilks,
    BSilks,
    FFab,
    BFab,
    /// Edge.Cuts — board outline. The courtyard polygon also draws
    /// here in the MVP because we don't yet have a separate
    /// `F.CrtYd` toggle.
    EdgeCuts,
}

impl FpLayer {
    /// Display order — left to right along the layer toolbar.
    pub const ORDER: &'static [FpLayer] = &[
        FpLayer::FCu,
        FpLayer::BCu,
        FpLayer::FSilks,
        FpLayer::BSilks,
        FpLayer::FFab,
        FpLayer::BFab,
        FpLayer::EdgeCuts,
    ];

    /// Short display label for the toolbar pill — Altium nomenclature
    /// per `docs/UX_REFERENCE_ALTIUM.md`. The Standard/data-layer name is
    /// available via [`Self::standard_name`] for sexpr round-trips.
    pub fn label(self) -> &'static str {
        match self {
            FpLayer::FCu => "Top Layer",
            FpLayer::BCu => "Bottom Layer",
            FpLayer::FSilks => "Top Overlay",
            FpLayer::BSilks => "Bottom Overlay",
            FpLayer::FFab => "Top Assembly",
            FpLayer::BFab => "Bottom Assembly",
            FpLayer::EdgeCuts => "Keep-Out",
        }
    }

    /// Standard layer-name string used in `.standard_mod` / footprint S-expressions.
    pub fn standard_name(self) -> &'static str {
        match self {
            FpLayer::FCu => "F.Cu",
            FpLayer::BCu => "B.Cu",
            FpLayer::FSilks => "F.SilkS",
            FpLayer::BSilks => "B.SilkS",
            FpLayer::FFab => "F.Fab",
            FpLayer::BFab => "B.Fab",
            FpLayer::EdgeCuts => "Edge.Cuts",
        }
    }

    /// Convert a Standard layer-name string into our enum, returning
    /// `None` for any layer the MVP doesn't expose. Used when parsing
    /// a footprint sexpr — graphics on unknown layers are still drawn
    /// (they end up routed to a sensible default colour) but the
    /// toolbar can't toggle them.
    pub fn from_standard_name(name: &str) -> Option<FpLayer> {
        match name {
            "F.Cu" => Some(FpLayer::FCu),
            "B.Cu" => Some(FpLayer::BCu),
            "F.SilkS" => Some(FpLayer::FSilks),
            "B.SilkS" => Some(FpLayer::BSilks),
            "F.Fab" => Some(FpLayer::FFab),
            "B.Fab" => Some(FpLayer::BFab),
            "Edge.Cuts" => Some(FpLayer::EdgeCuts),
            _ => None,
        }
    }

    /// Render colour for the layer — Altium-flavoured palette, kept
    /// muted so multiple visible layers don't drown each other out.
    pub fn color(self) -> Color {
        match self {
            FpLayer::FCu => Color::from_rgba(0.85, 0.20, 0.20, 1.0),
            FpLayer::BCu => Color::from_rgba(0.30, 0.45, 0.95, 1.0),
            FpLayer::FSilks => Color::from_rgba(0.95, 0.95, 0.95, 1.0),
            FpLayer::BSilks => Color::from_rgba(0.65, 0.55, 0.85, 1.0),
            FpLayer::FFab => Color::from_rgba(0.85, 0.65, 0.30, 1.0),
            FpLayer::BFab => Color::from_rgba(0.55, 0.45, 0.30, 1.0),
            FpLayer::EdgeCuts => Color::from_rgba(0.95, 0.85, 0.20, 1.0),
        }
    }
}

/// Which colour set the footprint canvas and layer widgets paint with.
/// Graphite themes get the Graphite values; every other theme keeps the
/// Altium-flavoured set of [`FpLayer::color`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerPalette {
    #[default]
    Legacy,
    Graphite,
    /// Graphite colours tuned for the paper canvas (dark ink on light).
    CanvasLight,
    /// Graphite colours tuned for the charcoal canvas.
    CanvasDark,
}

impl LayerPalette {
    /// Outline colour of a selected pad or silk graphic.
    pub fn selection_ink(self) -> Color {
        match self {
            LayerPalette::CanvasLight => Color::from_rgb8(0x15, 0x17, 0x1A),
            _ => Color::WHITE,
        }
    }

    /// The palette a theme paints with.
    pub fn for_tokens(tokens: &signex_types::theme::ThemeTokens) -> Self {
        if crate::styles::is_graphite(tokens) {
            Self::Graphite
        } else {
            Self::Legacy
        }
    }
}

/// The board side a layer sits on; drives the grouping of the layers panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpSide {
    Top,
    Bottom,
    Board,
}

impl FpSide {
    /// Display order of the groups in the layers panel.
    pub const ORDER: &'static [FpSide] = &[FpSide::Top, FpSide::Bottom, FpSide::Board];

    pub fn label(self) -> &'static str {
        match self {
            FpSide::Top => "Top side",
            FpSide::Bottom => "Bottom side",
            FpSide::Board => "Board",
        }
    }
}

impl FpLayer {
    /// The side of the board this layer belongs to.
    pub fn side(self) -> FpSide {
        match self {
            FpLayer::FCu | FpLayer::FSilks | FpLayer::FFab => FpSide::Top,
            FpLayer::BCu | FpLayer::BSilks | FpLayer::BFab => FpSide::Bottom,
            FpLayer::EdgeCuts => FpSide::Board,
        }
    }

    /// Colour in `palette`. Graphite: pad gold on top copper, blue on
    /// bottom copper, warm white silk, board-outline yellow. The canvas
    /// variants put the courtyard (drawn with the Edge.Cuts colour) in
    /// magenta and keep silk readable on their background.
    pub fn color_in(self, palette: LayerPalette) -> Color {
        match palette {
            LayerPalette::Legacy => self.color(),
            LayerPalette::CanvasLight => match self {
                FpLayer::FSilks => Color::from_rgb8(0x2B, 0x2D, 0x33),
                FpLayer::BSilks => Color::from_rgb8(0x7A, 0x75, 0x68),
                FpLayer::FFab => Color::from_rgb8(0x2F, 0x7F, 0x90),
                FpLayer::BFab => Color::from_rgb8(0x3D, 0x7F, 0x8D),
                FpLayer::EdgeCuts => Color::from_rgb8(0xB5, 0x47, 0x8A),
                _ => self.color_in(LayerPalette::Graphite),
            },
            LayerPalette::CanvasDark => match self {
                FpLayer::FFab => Color::from_rgb8(0x4F, 0xA6, 0xB8),
                FpLayer::BFab => Color::from_rgb8(0x3D, 0x7F, 0x8D),
                FpLayer::EdgeCuts => Color::from_rgb8(0xD1, 0x6B, 0xA5),
                _ => self.color_in(LayerPalette::Graphite),
            },
            LayerPalette::Graphite => match self {
                FpLayer::FCu => Color::from_rgb8(0xD8, 0xB4, 0x6A),
                FpLayer::BCu => Color::from_rgb8(0x3B, 0x6F, 0xD8),
                FpLayer::FSilks => Color::from_rgb8(0xF2, 0xEF, 0xE6),
                FpLayer::BSilks => Color::from_rgb8(0xA8, 0xA2, 0xC8),
                FpLayer::FFab => Color::from_rgb8(0x9E, 0xA3, 0xAE),
                FpLayer::BFab => Color::from_rgb8(0x6F, 0x73, 0x7C),
                FpLayer::EdgeCuts => Color::from_rgb8(0xE7, 0xD9, 0xA8),
            },
        }
    }
}

/// Per-layer visibility map — a 7-entry struct we index by `FpLayer`
/// rather than a `HashMap` so the footprint state is `Clone +
/// PartialEq` without a derive dance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerVisibility {
    pub f_cu: bool,
    pub b_cu: bool,
    pub f_silks: bool,
    pub b_silks: bool,
    pub f_fab: bool,
    pub b_fab: bool,
    pub edge_cuts: bool,
}

impl Default for LayerVisibility {
    fn default() -> Self {
        // Front-side layers on by default. Back layers off so a freshly
        // opened SMD footprint doesn't render duplicated silk/fab over
        // the front view.
        Self {
            f_cu: true,
            b_cu: false,
            f_silks: true,
            b_silks: false,
            f_fab: true,
            b_fab: false,
            edge_cuts: true,
        }
    }
}

impl LayerVisibility {
    pub fn get(&self, layer: FpLayer) -> bool {
        match layer {
            FpLayer::FCu => self.f_cu,
            FpLayer::BCu => self.b_cu,
            FpLayer::FSilks => self.f_silks,
            FpLayer::BSilks => self.b_silks,
            FpLayer::FFab => self.f_fab,
            FpLayer::BFab => self.b_fab,
            FpLayer::EdgeCuts => self.edge_cuts,
        }
    }

    /// Every layer visible.
    pub fn show_all(&mut self) {
        *self = Self {
            f_cu: true,
            b_cu: true,
            f_silks: true,
            b_silks: true,
            f_fab: true,
            b_fab: true,
            edge_cuts: true,
        };
    }

    /// Top-side layers and the board outline visible, bottom side hidden.
    pub fn show_top_side_only(&mut self) {
        *self = Self {
            f_cu: true,
            b_cu: false,
            f_silks: true,
            b_silks: false,
            f_fab: true,
            b_fab: false,
            edge_cuts: true,
        };
    }

    pub fn toggle(&mut self, layer: FpLayer) {
        match layer {
            FpLayer::FCu => self.f_cu = !self.f_cu,
            FpLayer::BCu => self.b_cu = !self.b_cu,
            FpLayer::FSilks => self.f_silks = !self.f_silks,
            FpLayer::BSilks => self.b_silks = !self.b_silks,
            FpLayer::FFab => self.f_fab = !self.f_fab,
            FpLayer::BFab => self.b_fab = !self.b_fab,
            FpLayer::EdgeCuts => self.edge_cuts = !self.edge_cuts,
        }
    }
}
