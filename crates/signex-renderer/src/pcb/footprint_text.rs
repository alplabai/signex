//! Footprint property texts (designator, comment) as the board draws them
//! (#2368): placed, layered, sized and justified by `pcb-geom` (#2434); this
//! module only keeps the ones the view shows.

use pcb_geom::BoardTextField;
use signex_types::designator::{DesignatorStandard, display_reference};
use signex_types::pcb::Footprint;
use signex_types::pcb_layer::PcbLayer;
use signex_types::schematic::HAlign;

use super::PcbSnapshot;
use super::layer_view::PcbLayerView;

/// One footprint text to draw.
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
    /// Where the anchor sits along the line: its left end, middle or right
    /// end, in the text's own (mirrored) frame.
    pub justify: HAlign,
    /// A footprint's designator, which the project's designator standard
    /// may show differently from the stored text (#1416).
    pub designator: bool,
}

/// The designator and comment texts of `footprint` that `view` shows: not
/// hidden, not empty, on a shown layer. Parameters are not drawn.
pub(super) fn footprint_texts(
    footprint: &Footprint,
    view: Option<&PcbLayerView>,
) -> Vec<FootprintTextInput> {
    pcb_geom::footprint_texts(footprint)
        .into_iter()
        .filter(|text| view.is_none_or(|v| v.is_visible(text.layer)))
        .map(|text| {
            let anchor = text.placement.anchor.to_mm();
            FootprintTextInput {
                content: text.content,
                position: [anchor.x as f32, anchor.y as f32],
                rotation_deg: text.placement.rotation_deg as f32,
                size_mm: text.size_mm as f32,
                mirrored: text.placement.mirrored,
                layer: text.layer,
                justify: text.justify,
                designator: text.field == BoardTextField::Designator,
            }
        })
        .collect()
}

impl PcbSnapshot {
    /// The designators drawn as `standard` shows them (`-QAC1` under EN IEC
    /// 81346-2 full class codes, #1416); the board keeps the stored text.
    pub fn with_designator_standard(mut self, standard: Option<DesignatorStandard>) -> Self {
        for text in self.texts.iter_mut().filter(|text| text.designator) {
            text.content = display_reference(standard, &text.content).into_owned();
        }
        self
    }
}

#[cfg(test)]
#[path = "footprint_text_tests.rs"]
mod tests;
