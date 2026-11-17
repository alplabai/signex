//! Kinds and named sets of a board's layers (#2394): what Hide layers ▸ and
//! Show layers ▸ act on, and the built-in sets of the layer set button.

use signex_types::pcb::LayerDef;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use super::layer_rows::{LayerGroup, board_layers, group_layers, layer_rows};

/// A kind of layer a tab menu hides or shows at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerKind {
    Signal,
    Plane,
    /// Silkscreen and designators.
    Overlay,
    /// Solder mask and adhesive.
    Mask,
    Paste,
    /// The board outline, documentation and the board's mechanical layers.
    Mechanical,
    All,
}

impl LayerKind {
    pub const ALL: [LayerKind; 7] = [
        Self::Signal,
        Self::Plane,
        Self::Overlay,
        Self::Mask,
        Self::Paste,
        Self::Mechanical,
        Self::All,
    ];

    /// The token a command operand names the kind by.
    pub fn token(self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Plane => "plane",
            Self::Overlay => "overlay",
            Self::Mask => "mask",
            Self::Paste => "paste",
            Self::Mechanical => "mechanical",
            Self::All => "all",
        }
    }

    /// The kind `token` names, ignoring case.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| token.trim().eq_ignore_ascii_case(kind.token()))
    }
}

/// A set of layers every board has: the first rows of the layer set button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinLayerSet {
    All,
    Signal,
    Plane,
    /// Every layer but the copper.
    NonSignal,
    Mechanical,
}

impl BuiltinLayerSet {
    pub const ALL: [BuiltinLayerSet; 5] = [
        Self::All,
        Self::Signal,
        Self::Plane,
        Self::NonSignal,
        Self::Mechanical,
    ];

    /// The token a command operand names the set by.
    pub fn token(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Signal => "signal",
            Self::Plane => "plane",
            Self::NonSignal => "non_signal",
            Self::Mechanical => "mechanical",
        }
    }

    /// The set `token` names, ignoring case.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|set| token.trim().eq_ignore_ascii_case(set.token()))
    }
}

/// Which set a board view shows: a built-in one or one the user saved.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LayerSetId {
    Builtin(BuiltinLayerSet),
    Saved(String),
}

/// The set a board view shows and its layers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownLayerSet {
    pub id: LayerSetId,
    pub layers: Vec<PcbLayer>,
}

/// Every layer the Layers panel lists for the board, and the board's own
/// layers: what a set or Show only this layer hides.
pub fn listed_layers(layers: &[LayerDef]) -> Vec<PcbLayer> {
    let mut out: Vec<PcbLayer> = layer_rows(layers)
        .into_iter()
        .map(|row| row.layer)
        .collect();
    for layer in board_layers(layers) {
        if !out.contains(&layer) {
            out.push(layer);
        }
    }
    out
}

fn is_paste(layer: PcbLayer) -> bool {
    matches!(
        layer,
        PcbLayer::Technical(TechnicalLayer::PasteTop | TechnicalLayer::PasteBottom)
    )
}

/// The layers of `kind` on a board.
pub fn kind_layers(layers: &[LayerDef], kind: LayerKind) -> Vec<PcbLayer> {
    match kind {
        LayerKind::Signal => group_layers(layers, LayerGroup::Signal),
        LayerKind::Plane => group_layers(layers, LayerGroup::Plane),
        LayerKind::Overlay => group_layers(layers, LayerGroup::Overlay),
        LayerKind::Mask => group_layers(layers, LayerGroup::Mask)
            .into_iter()
            .filter(|&layer| !is_paste(layer))
            .collect(),
        LayerKind::Paste => group_layers(layers, LayerGroup::Mask)
            .into_iter()
            .filter(|&layer| is_paste(layer))
            .collect(),
        LayerKind::Mechanical => group_layers(layers, LayerGroup::Mechanical),
        LayerKind::All => listed_layers(layers),
    }
}

/// The layers of a built-in set on a board.
pub fn builtin_set_layers(layers: &[LayerDef], set: BuiltinLayerSet) -> Vec<PcbLayer> {
    match set {
        BuiltinLayerSet::All => listed_layers(layers),
        BuiltinLayerSet::Signal => kind_layers(layers, LayerKind::Signal),
        BuiltinLayerSet::Plane => kind_layers(layers, LayerKind::Plane),
        BuiltinLayerSet::NonSignal => listed_layers(layers)
            .into_iter()
            .filter(|layer| !layer.is_copper())
            .collect(),
        BuiltinLayerSet::Mechanical => kind_layers(layers, LayerKind::Mechanical),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::CopperLayer;

    fn def(id: u8, name: &str, kind: &str) -> LayerDef {
        LayerDef {
            id,
            name: name.to_owned(),
            layer_type: kind.to_owned(),
            display_name: String::new(),
        }
    }

    #[test]
    fn tokens_round_trip_and_ignore_case() {
        for kind in LayerKind::ALL {
            assert_eq!(LayerKind::from_token(kind.token()), Some(kind));
        }
        for set in BuiltinLayerSet::ALL {
            assert_eq!(BuiltinLayerSet::from_token(set.token()), Some(set));
        }
        assert_eq!(
            BuiltinLayerSet::from_token(" Signal "),
            Some(BuiltinLayerSet::Signal)
        );
        assert_eq!(LayerKind::from_token("copper"), None);
    }

    #[test]
    fn paste_is_its_own_kind_and_non_signal_has_no_copper() {
        let layers = [
            def(0, "TopCopper", "signal"),
            def(1, "Inner1Copper", "plane"),
            def(31, "BottomCopper", "signal"),
        ];
        let paste = kind_layers(&layers, LayerKind::Paste);
        assert_eq!(
            paste,
            [
                PcbLayer::Technical(TechnicalLayer::PasteTop),
                PcbLayer::Technical(TechnicalLayer::PasteBottom)
            ]
        );
        assert!(
            kind_layers(&layers, LayerKind::Mask)
                .iter()
                .all(|&l| !is_paste(l))
        );
        let plane = PcbLayer::Copper(CopperLayer::Inner(
            signex_types::pcb_layer::InnerCopper::new(1).expect("inner 1"),
        ));
        assert_eq!(builtin_set_layers(&layers, BuiltinLayerSet::Plane), [plane]);
        let non_signal = builtin_set_layers(&layers, BuiltinLayerSet::NonSignal);
        assert!(!non_signal.is_empty() && non_signal.iter().all(|l| !l.is_copper()));
    }
}
