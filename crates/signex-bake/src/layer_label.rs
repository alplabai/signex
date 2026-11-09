//! Display label a baked footprint carries for a sketch layer.
//!
//! The label set is the Altium-style one ("Top Layer", "Top Overlay", …)
//! the baked footprint format stores in [`LayerId`]. Only the layers a
//! sketch attribute can name are covered; any other layer falls back to
//! its on-disk token, so a label is never empty.

use signex_library::primitive::footprint::LayerId;
use signex_types::format::pcb_layer_token;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer as T};

/// The baked-footprint label of `layer`.
pub fn layer_label(layer: PcbLayer) -> String {
    match layer {
        PcbLayer::Copper(CopperLayer::Top) => "Top Layer".into(),
        PcbLayer::Copper(CopperLayer::Bottom) => "Bottom Layer".into(),
        PcbLayer::Copper(CopperLayer::Inner(n)) => format!("Mid Layer {}", n.index()),
        PcbLayer::Technical(T::SilkscreenTop) => "Top Overlay".into(),
        PcbLayer::Technical(T::SilkscreenBottom) => "Bottom Overlay".into(),
        PcbLayer::Technical(T::SolderMaskTop) => "Top Solder".into(),
        PcbLayer::Technical(T::SolderMaskBottom) => "Bottom Solder".into(),
        PcbLayer::Technical(T::PasteTop) => "Top Paste".into(),
        PcbLayer::Technical(T::PasteBottom) => "Bottom Paste".into(),
        PcbLayer::Technical(T::AssemblyTop) => "Top Assembly".into(),
        PcbLayer::Technical(T::AssemblyBottom) => "Bottom Assembly".into(),
        PcbLayer::Technical(T::CourtyardTop) => "Top Courtyard".into(),
        PcbLayer::Technical(T::CourtyardBottom) => "Bottom Courtyard".into(),
        PcbLayer::Technical(T::EdgeCuts) => "Board Outline".into(),
        PcbLayer::Mechanical(n) => format!("Mechanical {}", n.index()),
        other => pcb_layer_token(other),
    }
}

/// [`layer_label`] as the string-typed wrapper a baked footprint stores.
pub fn layer_id(layer: PcbLayer) -> LayerId {
    LayerId::new(layer_label(layer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::{InnerCopper, MechanicalLayer};

    /// Every string the retired `SignexLayer::altium_label` produced for a
    /// layer a sketch can name. Baked output must stay byte-identical.
    #[test]
    fn labels_are_the_strings_baked_footprints_always_carried() {
        let tech = PcbLayer::Technical;
        let cases = [
            (PcbLayer::Copper(CopperLayer::Top), "Top Layer"),
            (PcbLayer::Copper(CopperLayer::Bottom), "Bottom Layer"),
            (
                PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(2).unwrap())),
                "Mid Layer 2",
            ),
            (tech(T::SilkscreenTop), "Top Overlay"),
            (tech(T::SilkscreenBottom), "Bottom Overlay"),
            (tech(T::SolderMaskTop), "Top Solder"),
            (tech(T::SolderMaskBottom), "Bottom Solder"),
            (tech(T::PasteTop), "Top Paste"),
            (tech(T::PasteBottom), "Bottom Paste"),
            (tech(T::AssemblyTop), "Top Assembly"),
            (tech(T::AssemblyBottom), "Bottom Assembly"),
            (tech(T::CourtyardTop), "Top Courtyard"),
            (tech(T::CourtyardBottom), "Bottom Courtyard"),
            (tech(T::EdgeCuts), "Board Outline"),
            (
                PcbLayer::Mechanical(MechanicalLayer::new(13).unwrap()),
                "Mechanical 13",
            ),
        ];
        for (layer, want) in cases {
            assert_eq!(layer_label(layer), want, "{layer:?}");
        }
    }

    #[test]
    fn a_layer_without_a_label_falls_back_to_its_token() {
        assert_eq!(
            layer_label(PcbLayer::Technical(T::KeepoutTop)),
            "TopKeepout"
        );
    }
}
