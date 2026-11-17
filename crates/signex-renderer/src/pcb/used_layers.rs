//! The layers a board has objects on (#2363): what the Layers panel's
//! Used On filter keeps.

use std::collections::HashSet;

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;

use super::BOARD_OUTLINE;
use super::footprint_text::footprint_texts;

/// Every layer `board` draws something on: its tracks, arcs, vias, pads,
/// zones, drawings, texts and dimensions, and the Board Outline when the
/// board has an outline.
pub fn used_layers(board: &PcbBoard) -> HashSet<PcbLayer> {
    let mut names: Vec<&str> = Vec::new();
    names.extend(board.segments.iter().map(|s| s.layer.as_str()));
    names.extend(board.free.arcs.iter().map(|a| a.layer.as_str()));
    names.extend(board.free.dimensions.iter().map(|d| d.layer.as_str()));
    names.extend(board.vias.iter().flat_map(|v| v.layers.iter().map(String::as_str)));
    names.extend(board.zones.iter().map(|z| z.layer.as_str()));
    names.extend(board.graphics.iter().map(|g| g.layer.as_str()));
    names.extend(board.texts.iter().map(|t| t.layer.as_str()));
    names.extend(
        board
            .free
            .pads
            .iter()
            .flat_map(|p| p.layers.iter().map(String::as_str)),
    );
    for footprint in &board.footprints {
        names.push(footprint.layer.as_str());
        names.extend(footprint.graphics.iter().map(|g| g.layer.as_str()));
        names.extend(
            footprint
                .pads
                .iter()
                .flat_map(|p| p.layers.iter().map(String::as_str)),
        );
    }
    let mut used: HashSet<PcbLayer> = names.into_iter().filter_map(parse_pcb_layer).collect();
    for footprint in &board.footprints {
        used.extend(footprint_texts(footprint, None).into_iter().map(|t| t.layer));
    }
    if !board.outline.is_empty() {
        used.insert(BOARD_OUTLINE);
    }
    used
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::{CopperLayer, TechnicalLayer};

    #[test]
    fn a_track_marks_its_layer_used_and_nothing_else() {
        let board: PcbBoard = serde_json::from_str(
            r#"{
              "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
              "generator": "test", "thickness": 1.6, "outline": [],
              "layers": [], "setup": null, "nets": [], "footprints": [],
              "segments": [
                { "uuid": "f5ea9948-4c2f-4501-b647-4047f6eefca9", "start": {"x":0.0,"y":0.0}, "end": {"x":5.0,"y":0.0}, "width": 0.25, "layer": "BottomCopper", "net": 1 }
              ],
              "vias": [], "zones": [], "graphics": [], "texts": []
            }"#,
        )
        .expect("valid board");
        let used = used_layers(&board);
        assert!(used.contains(&PcbLayer::Copper(CopperLayer::Bottom)));
        assert!(!used.contains(&PcbLayer::Copper(CopperLayer::Top)));
        assert!(!used.contains(&PcbLayer::Technical(TechnicalLayer::SilkscreenTop)));
        assert!(!used.contains(&BOARD_OUTLINE), "no outline drawn");
    }
}
