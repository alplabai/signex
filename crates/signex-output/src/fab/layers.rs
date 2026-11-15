//! Which fabrication file a board layer goes to, the copper stack, and the
//! file names and X2 file functions.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer, TechnicalLayer};

/// One Gerber image: a copper layer, a side's legend, mask or paste, or
/// the board profile. Ordered the way the files are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FabLayer {
    Copper(CopperLayer),
    Legend(Face),
    Mask(Face),
    Paste(Face),
    Profile,
}

/// A board face, ordered top first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Face {
    Top,
    Bottom,
}

impl From<BoardSide> for Face {
    fn from(side: BoardSide) -> Self {
        match side {
            BoardSide::Top => Face::Top,
            BoardSide::Bottom => Face::Bottom,
        }
    }
}

impl Face {
    fn word(self) -> &'static str {
        match self {
            Face::Top => "Top",
            Face::Bottom => "Bot",
        }
    }

    fn file_word(self) -> &'static str {
        match self {
            Face::Top => "top",
            Face::Bottom => "bottom",
        }
    }
}

impl FabLayer {
    /// The image a board layer goes to; `None` for a layer that is not part
    /// of the fabrication data (assembly, courtyard, documentation, ...).
    pub(crate) fn of(layer: PcbLayer) -> Option<FabLayer> {
        use TechnicalLayer as T;
        match layer {
            PcbLayer::Copper(copper) => Some(FabLayer::Copper(copper)),
            PcbLayer::Technical(T::SilkscreenTop) => Some(FabLayer::Legend(Face::Top)),
            PcbLayer::Technical(T::SilkscreenBottom) => Some(FabLayer::Legend(Face::Bottom)),
            PcbLayer::Technical(T::SolderMaskTop) => Some(FabLayer::Mask(Face::Top)),
            PcbLayer::Technical(T::SolderMaskBottom) => Some(FabLayer::Mask(Face::Bottom)),
            PcbLayer::Technical(T::PasteTop) => Some(FabLayer::Paste(Face::Top)),
            PcbLayer::Technical(T::PasteBottom) => Some(FabLayer::Paste(Face::Bottom)),
            PcbLayer::Technical(T::EdgeCuts) => Some(FabLayer::Profile),
            _ => None,
        }
    }

    /// The image a layer token goes to; `None` for an unknown token too.
    pub(crate) fn of_token(token: &str) -> Option<FabLayer> {
        parse_pcb_layer(token).and_then(FabLayer::of)
    }

    pub(crate) fn is_copper(self) -> bool {
        matches!(self, FabLayer::Copper(_))
    }
}

/// The board's copper layers top to bottom: top and bottom always, every
/// inner layer the stack lists, and any inner layer an item is drawn on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CopperStack {
    layers: Vec<CopperLayer>,
}

impl CopperStack {
    pub(crate) fn of(board: &PcbBoard, used: impl Iterator<Item = CopperLayer>) -> Self {
        let mut layers = vec![CopperLayer::Top, CopperLayer::Bottom];
        let listed = board
            .layers
            .iter()
            .filter_map(|def| def.pcb_layer())
            .filter_map(PcbLayer::copper);
        for layer in listed.chain(used) {
            if !layers.contains(&layer) {
                layers.push(layer);
            }
        }
        layers.sort();
        Self { layers }
    }

    pub(crate) fn layers(&self) -> &[CopperLayer] {
        &self.layers
    }

    pub(crate) fn count(&self) -> usize {
        self.layers.len()
    }

    /// 1-based position of `layer` in the stack, top first.
    pub(crate) fn number(&self, layer: CopperLayer) -> Option<usize> {
        self.layers.iter().position(|l| *l == layer).map(|i| i + 1)
    }

    pub(crate) fn contains(&self, layer: CopperLayer) -> bool {
        self.layers.contains(&layer)
    }
}

/// The X2 `.FileFunction` value of an image.
pub(crate) fn file_function(layer: FabLayer, stack: &CopperStack) -> String {
    match layer {
        FabLayer::Copper(copper) => {
            let n = stack.number(copper).unwrap_or(0);
            let kind = match copper {
                CopperLayer::Top => "Top",
                CopperLayer::Bottom => "Bot",
                CopperLayer::Inner(_) => "Inr",
            };
            format!("Copper,L{n},{kind}")
        }
        FabLayer::Legend(face) => format!("Legend,{}", face.word()),
        FabLayer::Mask(face) => format!("Soldermask,{}", face.word()),
        FabLayer::Paste(face) => format!("Paste,{}", face.word()),
        FabLayer::Profile => "Profile,NP".to_string(),
    }
}

/// The X2 `.FilePolarity` of an image: a mask image draws the openings.
pub(crate) fn file_polarity(layer: FabLayer) -> &'static str {
    match layer {
        FabLayer::Mask(_) => "Negative",
        _ => "Positive",
    }
}

/// The file-name suffix of an image, before `.gbr`.
pub(crate) fn file_suffix(layer: FabLayer, stack: &CopperStack) -> String {
    match layer {
        FabLayer::Copper(copper) => {
            let n = stack.number(copper).unwrap_or(0);
            let place = match copper {
                CopperLayer::Top => "top".to_string(),
                CopperLayer::Bottom => "bottom".to_string(),
                CopperLayer::Inner(inner) => format!("inner{}", inner.index()),
            };
            format!("copper-l{n}-{place}")
        }
        FabLayer::Legend(face) => format!("legend-{}", face.file_word()),
        FabLayer::Mask(face) => format!("soldermask-{}", face.file_word()),
        FabLayer::Paste(face) => format!("paste-{}", face.file_word()),
        FabLayer::Profile => "profile".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::InnerCopper;

    fn inner(n: u8) -> CopperLayer {
        CopperLayer::Inner(InnerCopper::new(n).unwrap())
    }

    #[test]
    fn the_stack_numbers_top_first_and_keeps_used_inner_layers() {
        let board = super::super::tests::fixture::empty_board();
        let stack = CopperStack::of(&board, [inner(2)].into_iter());
        assert_eq!(
            stack.layers(),
            &[CopperLayer::Top, inner(2), CopperLayer::Bottom]
        );
        assert_eq!(stack.number(CopperLayer::Bottom), Some(3));
        assert_eq!(
            file_function(FabLayer::Copper(inner(2)), &stack),
            "Copper,L2,Inr"
        );
        assert_eq!(
            file_suffix(FabLayer::Copper(inner(2)), &stack),
            "copper-l2-inner2"
        );
    }

    #[test]
    fn only_fabrication_layers_have_an_image() {
        use TechnicalLayer as T;
        assert_eq!(
            FabLayer::of(PcbLayer::Technical(T::SolderMaskBottom)),
            Some(FabLayer::Mask(Face::Bottom))
        );
        assert_eq!(FabLayer::of(PcbLayer::Technical(T::CourtyardTop)), None);
        assert_eq!(FabLayer::of_token("EdgeCuts"), Some(FabLayer::Profile));
        assert_eq!(FabLayer::of_token("NoSuchLayer"), None);
    }
}
