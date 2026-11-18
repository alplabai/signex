//! The Component Layer Pairs of the Layers panel (#2363): one row per kind
//! of component layer, its top layer on the left and its bottom layer on
//! the right. A row Signex has no layer for is listed greyed out.

use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

/// A row of the Component Layer Pairs table, in the order it lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerPair {
    Body3d,
    Assembly,
    Coating,
    ComponentCenter,
    ComponentOutline,
    Courtyard,
    Designator,
    Overlay,
    Paste,
    Solder,
    Tenting,
    Value,
    Adhesive,
    Fiducial,
    Keepout,
}

impl LayerPair {
    pub const ALL: [LayerPair; 15] = [
        Self::Body3d,
        Self::Assembly,
        Self::Coating,
        Self::ComponentCenter,
        Self::ComponentOutline,
        Self::Courtyard,
        Self::Designator,
        Self::Overlay,
        Self::Paste,
        Self::Solder,
        Self::Tenting,
        Self::Value,
        Self::Adhesive,
        Self::Fiducial,
        Self::Keepout,
    ];

    /// Its top and bottom layer; `None` for a row Signex has no layer for.
    pub fn layers(self) -> Option<(PcbLayer, PcbLayer)> {
        use TechnicalLayer as T;
        let (top, bottom) = match self {
            Self::Body3d => (T::Body3dTop, T::Body3dBottom),
            Self::Assembly => (T::AssemblyTop, T::AssemblyBottom),
            // The fabrication drawing carries the component outlines.
            Self::ComponentOutline => (T::FabTop, T::FabBottom),
            Self::Courtyard => (T::CourtyardTop, T::CourtyardBottom),
            Self::Designator => (T::DesignatorTop, T::DesignatorBottom),
            Self::Overlay => (T::SilkscreenTop, T::SilkscreenBottom),
            Self::Paste => (T::PasteTop, T::PasteBottom),
            Self::Solder => (T::SolderMaskTop, T::SolderMaskBottom),
            Self::Adhesive => (T::AdhesiveTop, T::AdhesiveBottom),
            Self::Fiducial => (T::FiducialTop, T::FiducialBottom),
            Self::Keepout => (T::KeepoutTop, T::KeepoutBottom),
            Self::Coating | Self::ComponentCenter | Self::Tenting | Self::Value => return None,
        };
        Some((PcbLayer::Technical(top), PcbLayer::Technical(bottom)))
    }
}

/// The technical layers the panel lists one by one, not in a pair: the
/// board shape, the margin and the documentation layers. The drill drawing
/// lists with the Other Layers.
pub const SINGLE_TECHNICAL_LAYERS: [TechnicalLayer; 6] = [
    TechnicalLayer::EdgeCuts,
    TechnicalLayer::Margin,
    TechnicalLayer::Dimensions,
    TechnicalLayer::Notes,
    TechnicalLayer::Eco1,
    TechnicalLayer::Eco2,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every technical layer is listed exactly once: in a pair, alone, or
    /// as the drill drawing of the Other Layers.
    #[test]
    fn every_technical_layer_is_listed_once() {
        let mut listed: Vec<PcbLayer> = LayerPair::ALL
            .into_iter()
            .filter_map(LayerPair::layers)
            .flat_map(|(top, bottom)| [top, bottom])
            .collect();
        listed.extend(SINGLE_TECHNICAL_LAYERS.map(PcbLayer::Technical));
        listed.push(PcbLayer::Technical(TechnicalLayer::DrillDrawing));
        for layer in TechnicalLayer::ALL {
            let count = listed
                .iter()
                .filter(|l| **l == PcbLayer::Technical(layer))
                .count();
            assert_eq!(count, 1, "{layer:?}");
        }
        assert_eq!(listed.len(), TechnicalLayer::ALL.len());
    }

    #[test]
    fn a_pair_is_a_top_layer_and_its_bottom_layer() {
        use signex_types::pcb_layer::BoardSide;
        for (top, bottom) in LayerPair::ALL.into_iter().filter_map(LayerPair::layers) {
            assert_eq!(top.side(), Some(BoardSide::Top), "{top:?}");
            assert_eq!(bottom.side(), Some(BoardSide::Bottom), "{bottom:?}");
        }
    }
}
