//! Which layers a board has, and how the Layers panel groups them (#2223,
//! #2359): the rows of the panel, the layers of the strip under the canvas
//! and the order the layer keys step through.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::LayerDef;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

/// A group of the Layers panel, in the order the panel lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerGroup {
    /// Copper that carries tracks: Top, the Mid-Layers, Bottom.
    Signal,
    /// Copper the stackup marks as a plane.
    Plane,
    /// The board outline and the documentation, fabrication and keepout
    /// layers, then the board's mechanical layers.
    Mechanical,
    /// Silkscreen and designators.
    Overlay,
    /// Solder mask, paste and adhesive.
    Mask,
    /// The drill drawing; the panel lists the Multi-Layer row (vias and
    /// through-hole pads) here too.
    Drill,
}

impl LayerGroup {
    pub const ALL: [LayerGroup; 6] = [
        Self::Signal,
        Self::Plane,
        Self::Mechanical,
        Self::Overlay,
        Self::Mask,
        Self::Drill,
    ];
}

/// One row of the Layers panel. Its name is the app's to give (a catalog
/// message), except for a mechanical layer the user named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerRow {
    pub layer: PcbLayer,
    pub group: LayerGroup,
    /// The board's own name for a mechanical layer; `None` for every other row.
    pub user_name: Option<String>,
}

/// Whether the stackup marks this copper layer as a plane.
fn is_plane(def: &LayerDef) -> bool {
    ["plane", "power"]
        .iter()
        .any(|kind| def.layer_type.eq_ignore_ascii_case(kind))
}

fn technical_group(layer: TechnicalLayer) -> LayerGroup {
    use TechnicalLayer as T;
    match layer {
        T::SilkscreenTop | T::SilkscreenBottom | T::DesignatorTop | T::DesignatorBottom => {
            LayerGroup::Overlay
        }
        T::SolderMaskTop
        | T::SolderMaskBottom
        | T::PasteTop
        | T::PasteBottom
        | T::AdhesiveTop
        | T::AdhesiveBottom => LayerGroup::Mask,
        T::DrillDrawing => LayerGroup::Drill,
        _ => LayerGroup::Mechanical,
    }
}

/// The plane layers a board's stackup lists.
fn plane_layers(layers: &[LayerDef]) -> Vec<CopperLayer> {
    layers
        .iter()
        .filter(|def| is_plane(def))
        .filter_map(|def| parse_pcb_layer(&def.name)?.copper())
        .collect()
}

/// The rows to list for a board: its copper layers (signal, then plane),
/// every technical layer, and the mechanical layers its stackup lists, by
/// index.
pub fn layer_rows(layers: &[LayerDef]) -> Vec<LayerRow> {
    let row = |layer: PcbLayer, group| LayerRow {
        layer,
        group,
        user_name: None,
    };
    let planes = plane_layers(layers);
    let mut rows: Vec<LayerRow> = board_copper_layers(layers)
        .into_iter()
        .map(|c| {
            let group = if planes.contains(&c) {
                LayerGroup::Plane
            } else {
                LayerGroup::Signal
            };
            row(PcbLayer::Copper(c), group)
        })
        .collect();
    rows.extend(
        TechnicalLayer::ALL
            .into_iter()
            .map(|t| row(PcbLayer::Technical(t), technical_group(t))),
    );
    let mut mechanical: Vec<(u8, LayerRow)> = layers
        .iter()
        .filter_map(|def| match parse_pcb_layer(&def.name)? {
            PcbLayer::Mechanical(m) => Some((
                m.index(),
                LayerRow {
                    layer: PcbLayer::Mechanical(m),
                    group: LayerGroup::Mechanical,
                    user_name: (!def.display_name.is_empty()).then(|| def.display_name.clone()),
                },
            )),
            _ => None,
        })
        .collect();
    mechanical.sort_by_key(|(index, _)| *index);
    mechanical.dedup_by_key(|(index, _)| *index);
    rows.extend(mechanical.into_iter().map(|(_, row)| row));
    rows
}

/// The layers of one panel group of a board.
pub fn group_layers(layers: &[LayerDef], group: LayerGroup) -> Vec<PcbLayer> {
    layer_rows(layers)
        .into_iter()
        .filter(|row| row.group == group)
        .map(|row| row.layer)
        .collect()
}

/// The copper layers a board defines, top to bottom; Top and Bottom when
/// it lists none.
pub fn board_copper_layers(layers: &[LayerDef]) -> Vec<CopperLayer> {
    signex_types::pcb_stack::board_copper_layers(layers)
}

/// The layers a board lists, in its own order: the tabs of the strip under
/// the canvas and the order `+` and `-` step through. A board that lists no
/// copper still has Top and Bottom, first.
pub fn board_layers(layers: &[LayerDef]) -> Vec<PcbLayer> {
    let mut out: Vec<PcbLayer> = Vec::new();
    for layer in layers.iter().filter_map(|def| parse_pcb_layer(&def.name)) {
        if !out.contains(&layer) {
            out.push(layer);
        }
    }
    if !out.iter().any(|layer| layer.is_copper()) {
        let copper = board_copper_layers(&[]).into_iter().map(PcbLayer::Copper);
        out.splice(0..0, copper);
    }
    out
}

/// The signal layers of a board, top to bottom: the copper the next signal
/// layer key (`*`) steps through. Every copper layer when all are planes.
pub fn signal_layers(layers: &[LayerDef]) -> Vec<PcbLayer> {
    let planes = plane_layers(layers);
    let copper = board_copper_layers(layers);
    let signal: Vec<PcbLayer> = copper
        .iter()
        .filter(|c| !planes.contains(c))
        .map(|&c| PcbLayer::Copper(c))
        .collect();
    if signal.is_empty() {
        copper.into_iter().map(PcbLayer::Copper).collect()
    } else {
        signal
    }
}
