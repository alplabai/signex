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
