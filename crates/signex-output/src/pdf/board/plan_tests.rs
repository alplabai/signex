//! The page plan's per-page description: which bottom sets mirror and which
//! of the board's layers each page prints (Board Print / PDF's page cards).

use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::{BoardLayerSet, BoardPrintOptions, BoardPrintPages};

/// A board with the named layers, in that order.
fn board_with_layers(names: &[&str]) -> PcbBoard {
    let layers: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(id, name)| serde_json::json!({ "id": id, "name": name, "layer_type": "signal" }))
        .collect();
    serde_json::from_value(serde_json::json!({ "uuid": uuid::Uuid::nil(), "layers": layers }))
        .expect("a board")
}

fn with_sets(sets: &[BoardLayerSet]) -> BoardPrintOptions {
    BoardPrintOptions {
        sets: sets.to_vec(),
        ..BoardPrintOptions::default()
    }
}

#[test]
fn only_a_bottom_set_makes_mirroring_matter() {
    assert!(BoardPrintOptions::default().has_bottom_set());
    assert!(!with_sets(&[BoardLayerSet::Top, BoardLayerSet::Mechanical]).has_bottom_set());
    assert!(with_sets(&[BoardLayerSet::AssemblyBottom]).has_bottom_set());
    assert!(!with_sets(&[]).has_bottom_set());
}

#[test]
fn a_page_lists_the_boards_layers_of_its_sets_then_the_outline() {
    let board = board_with_layers(&[
        "TopCopper",
        "Inner1Copper",
        "BottomCopper",
        "TopSilkscreen",
        "BottomSilkscreen",
        "TopSolderMask",
        "EdgeCuts",
    ]);
    let mut options = with_sets(&[BoardLayerSet::Top]);
    options.outline = true;
    let top = options.page_layers(&board, &[BoardLayerSet::Top]);
    assert_eq!(
        top,
        [
            PcbLayer::Copper(CopperLayer::Top),
            PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
            PcbLayer::Technical(TechnicalLayer::SolderMaskTop),
            PcbLayer::Technical(TechnicalLayer::EdgeCuts),
        ]
    );
    options.outline = false;
    let top = options.page_layers(&board, &[BoardLayerSet::Top]);
    assert!(
        !top.contains(&PcbLayer::Technical(TechnicalLayer::EdgeCuts)),
        "no outline when it is not drawn: {top:?}"
    );
    let mechanical = options.page_layers(&board, &[BoardLayerSet::Mechanical]);
    assert_eq!(
        mechanical,
        [PcbLayer::Technical(TechnicalLayer::EdgeCuts)],
        "the outline layer is the mechanical set's own, listed once"
    );
}

#[test]
fn a_composite_page_lists_every_set_once_in_the_boards_order() {
    let board = board_with_layers(&["TopCopper", "Inner1Copper", "BottomCopper"]);
    let mut options = with_sets(&[BoardLayerSet::Top, BoardLayerSet::Inner]);
    options.pages = BoardPrintPages::Composite;
    options.outline = false;
    let plan = options.page_plan();
    assert_eq!(plan.len(), 1);
    let layers = options.page_layers(&board, &plan[0].0);
    assert_eq!(layers.len(), 2, "{layers:?}");
    assert_eq!(layers[0], PcbLayer::Copper(CopperLayer::Top));
}
