//! Footprint property texts (designator, comment) as the board draws them
//! (#2368): placed by `pcb-geom`, on the property's own layer.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, PCB_DEFAULT_TEXT_SIZE_MM};
use signex_types::pcb_layer::{BoardSide, PcbLayer, TechnicalLayer};
use signex_types::property::PcbProperty;

use super::layer_view::PcbLayerView;

/// One footprint text to draw, centred on its anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct FootprintTextInput {
    pub content: String,
    /// The anchor, board mm.
    pub position: [f32; 2],
    /// The baseline's angle with the board's x axis, degrees, in the sense
    /// a pad turns.
    pub rotation_deg: f32,
    /// Cap height, mm.
    pub size_mm: f32,
    /// Drawn mirrored: the footprint is on the bottom side.
    pub mirrored: bool,
    /// The layer it is drawn on, and in the colour of.
    pub layer: PcbLayer,
}

/// The designator and comment texts of `footprint` that `view` shows: not
/// hidden, not empty, on a shown layer. Parameters are not drawn.
pub(super) fn footprint_texts(
    footprint: &Footprint,
    view: Option<&PcbLayerView>,
) -> Vec<FootprintTextInput> {
    footprint
        .properties
        .iter()
        .filter(|property| pcb_geom::is_board_text_key(&property.key))
        .filter(|property| !property.hidden && !property.value.trim().is_empty())
        .filter_map(|property| {
            let layer = text_layer(footprint, property);
            if view.is_some_and(|v| !v.is_visible(layer)) {
                return None;
            }
            text_input(footprint, property, layer)
        })
        .collect()
}

/// The layer `property` is drawn on: its own, else the overlay of the
/// footprint's side.
fn text_layer(footprint: &Footprint, property: &PcbProperty) -> PcbLayer {
    if let Some(layer) = property.layer.as_deref().and_then(parse_pcb_layer) {
        return layer;
    }
    let side = parse_pcb_layer(&footprint.layer).and_then(PcbLayer::side);
    PcbLayer::Technical(match side {
        Some(BoardSide::Bottom) => TechnicalLayer::SilkscreenBottom,
        _ => TechnicalLayer::SilkscreenTop,
    })
}

fn text_input(
    footprint: &Footprint,
    property: &PcbProperty,
    layer: PcbLayer,
) -> Option<FootprintTextInput> {
    let placed = pcb_geom::property_text_placement(footprint, property).ok()?;
    let anchor = placed.anchor.to_mm();
    let size_mm = property
        .font_size
        .filter(|size| size.is_finite() && *size > 0.0)
        .unwrap_or(PCB_DEFAULT_TEXT_SIZE_MM);
    Some(FootprintTextInput {
        content: property.value.clone(),
        position: [anchor.x as f32, anchor.y as f32],
        rotation_deg: placed.rotation_deg as f32,
        size_mm: size_mm as f32,
        mirrored: placed.mirrored,
        layer,
    })
}

#[cfg(test)]
#[path = "footprint_text_tests.rs"]
mod tests;
