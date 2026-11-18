//! Footprint editor layer registry — the seven Altium-spec layers a
//! footprint surfaces: F.Cu / B.Cu / F.SilkS / B.SilkS / F.Fab / B.Fab
//! / Edge.Cuts (the courtyard polygon stays on Edge.Cuts in this Phase
//! 2 cut; F.CrtYd / B.CrtYd land alongside the proper paste/mask
//! toggles in v0.9.x).

use iced::Color;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

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

    /// The catalog message of [`Self::label`].
    pub fn message(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            FpLayer::FCu => ids::EDITORS_LAYER_TOP,
            FpLayer::BCu => ids::EDITORS_LAYER_BOTTOM,
            FpLayer::FSilks => ids::EDITORS_LAYER_TOP_OVERLAY,
            FpLayer::BSilks => ids::EDITORS_LAYER_BOTTOM_OVERLAY,
            FpLayer::FFab => ids::EDITORS_LAYER_TOP_ASSEMBLY,
            FpLayer::BFab => ids::EDITORS_LAYER_BOTTOM_ASSEMBLY,
            FpLayer::EdgeCuts => ids::EDITORS_LAYER_KEEP_OUT,
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

    /// The layer's colour: the colour of its board layer in the one board
    /// palette, so a footprint looks the same in the editor, the previews
    /// and on the board in every theme (#2375).
    pub fn color(self) -> Color {
        crate::pcb_canvas::layer_style::board_layer_colour(self.board_layer())
    }
}

/// How the footprint canvases draw the layers. The colours always come
/// from the board palette ([`FpLayer::color`]); the variant only picks the
/// drawing style around them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerPalette {
    /// The footprint editor: dashed courtyard, slightly see-through pads.
    #[default]
    Editor,
    /// Read-only previews: solid courtyard and opaque pads, as on the
    /// board (#2348).
    Board,
}

impl LayerPalette {
    /// Outline colour of a selected pad or silk graphic.
    pub fn selection_ink(self) -> Color {
        Color::WHITE
    }

    /// Whether the courtyard is drawn dashed (the editor canvas).
    pub fn dashes_courtyard(self) -> bool {
        self == LayerPalette::Editor
    }

    /// Colour of the courtyard outline: Courtyard Top, where the board
    /// puts it.
    pub fn courtyard_colour(self) -> Color {
        crate::pcb_canvas::layer_style::board_layer_colour(PcbLayer::Technical(
            TechnicalLayer::CourtyardTop,
        ))
    }

    /// Alpha of a pad's copper fill: opaque as on the board, slightly
    /// see-through in the editor.
    pub fn pad_fill_alpha(self) -> f32 {
        if self == LayerPalette::Board {
            1.0
        } else {
            0.85
        }
    }

    /// Edge stroke of a pad filled with `fill`: the fill colour itself.
    pub fn pad_edge(self, fill: Color) -> Color {
        Color { a: 1.0, ..fill }
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

    /// The group heading of the layers panel, written in capitals.
    pub fn heading(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            FpSide::Top => ids::EDITORS_LAYERS_SIDE_TOP,
            FpSide::Bottom => ids::EDITORS_LAYERS_SIDE_BOTTOM,
            FpSide::Board => ids::EDITORS_LAYERS_SIDE_BOARD,
        }
    }
}

impl FpLayer {
    /// The board layer this layer's graphics land on, as `signex-library`
    /// places a footprint (Edge.Cuts stays the board outline).
    pub fn board_layer(self) -> PcbLayer {
        use TechnicalLayer as T;
        match self {
            FpLayer::FCu => PcbLayer::Copper(CopperLayer::Top),
            FpLayer::BCu => PcbLayer::Copper(CopperLayer::Bottom),
            FpLayer::FSilks => PcbLayer::Technical(T::SilkscreenTop),
            FpLayer::BSilks => PcbLayer::Technical(T::SilkscreenBottom),
            FpLayer::FFab => PcbLayer::Technical(T::FabTop),
            FpLayer::BFab => PcbLayer::Technical(T::FabBottom),
            FpLayer::EdgeCuts => PcbLayer::Technical(T::EdgeCuts),
        }
    }

    /// The side of the board this layer belongs to.
    pub fn side(self) -> FpSide {
        match self {
            FpLayer::FCu | FpLayer::FSilks | FpLayer::FFab => FpSide::Top,
            FpLayer::BCu | FpLayer::BSilks | FpLayer::BFab => FpSide::Bottom,
            FpLayer::EdgeCuts => FpSide::Board,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb_canvas::layer_style::layer_colour;
    use signex_types::theme::{ThemeId, theme_tokens};

    /// The footprint editor draws each layer in the colour the PCB canvas
    /// gives its board layer, in both themes (#2375, #2468).
    #[test]
    fn editor_layer_colours_equal_the_pcb_canvas_colours() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            for layer in FpLayer::ORDER {
                assert_eq!(
                    layer.color(),
                    layer_colour(layer.board_layer(), &tokens),
                    "{layer:?} in {id:?}"
                );
            }
        }
    }

    /// Top copper is the board's red, not the gold of the old editor set.
    #[test]
    fn editor_top_copper_is_the_board_red() {
        let gold = Color::from_rgb8(0xD8, 0xB4, 0x6A);
        assert_ne!(FpLayer::FCu.color(), gold);
        assert_eq!(
            FpLayer::FCu.color(),
            Color::from_rgb8(0xD0, 0x40, 0x40),
            "the board palette's top copper"
        );
    }
}
