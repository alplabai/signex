//! The colour a board layer is drawn in, and listed with in the Layers
//! panel (#2387): one lookup for both, from the board palette.

use signex_gfx::style::ColorSlot;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::theme::{BoardLayerColors, Color};

use super::{BOARD_OUTLINE, copper_slot};
use crate::theme::{ResolvedTheme, to_rgba};

impl ResolvedTheme {
    /// The colour of `layer`. Copper, the Board Outline and the keep-out
    /// layers take the slots their own geometry is drawn in.
    pub fn board_layer_color(&self, layer: PcbLayer) -> [f32; 4] {
        use TechnicalLayer as T;
        let colors = self.board_layers();
        match layer {
            _ if layer == BOARD_OUTLINE => self.color(ColorSlot::BoardEdge),
            PcbLayer::Technical(T::KeepoutTop | T::KeepoutBottom) => self.color(ColorSlot::Keepout),
            PcbLayer::Copper(copper) => self.color(copper_slot(copper)),
            PcbLayer::Mechanical(m) => {
                to_rgba(colors.layer_cycle[usize::from(m.index()) % colors.layer_cycle.len()])
            }
            PcbLayer::Technical(t) => match technical_colour(colors, t) {
                Some(c) => to_rgba(c),
                None => self.color(ColorSlot::BoardEdge),
            },
        }
    }
}

/// A technical layer's own palette colour; `None` for the layers drawn in a
/// slot of their own.
fn technical_colour(colors: &BoardLayerColors, layer: TechnicalLayer) -> Option<Color> {
    use TechnicalLayer as T;
    let t = &colors.technical;
    Some(match layer {
        T::Margin => colors.margin,
        T::SilkscreenTop | T::SilkscreenBottom => colors.silkscreen,
        T::SolderMaskTop | T::SolderMaskBottom => colors.solder_mask,
        T::PasteTop | T::PasteBottom => colors.paste,
        T::AdhesiveTop => t.adhesive.top,
        T::AdhesiveBottom => t.adhesive.bottom,
        T::FabTop => t.fab.top,
        T::FabBottom => t.fab.bottom,
        T::AssemblyTop => t.assembly.top,
        T::AssemblyBottom => t.assembly.bottom,
        T::CourtyardTop => t.courtyard.top,
        T::CourtyardBottom => t.courtyard.bottom,
        T::Body3dTop => t.body_3d.top,
        T::Body3dBottom => t.body_3d.bottom,
        T::FiducialTop => t.fiducial.top,
        T::FiducialBottom => t.fiducial.bottom,
        T::DesignatorTop => t.designator.top,
        T::DesignatorBottom => t.designator.bottom,
        T::Dimensions => t.dimensions,
        T::Notes => t.notes,
        T::DrillDrawing => t.drill_drawing,
        T::Eco1 => t.eco_1,
        T::Eco2 => t.eco_2,
        T::EdgeCuts | T::KeepoutTop | T::KeepoutBottom => return None,
    })
}
