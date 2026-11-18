//! 2026-11-18: the layer tab strip lists every layer the Layers panel shows
//! that is not hidden, in the panel's order; the panel's eye and the strip
//! read one board view.

use super::*;
use signex_renderer::pcb::{LayerPair, SINGLE_TECHNICAL_LAYERS};
use signex_types::pcb_layer::{CopperLayer, TechnicalLayer};

/// A new two-layer board: it declares only a few technical layers.
fn board() -> PcbBoard {
    signex_types::project::SetupProfile::Hobby
        .pcb(2)
        .new_board()
}

#[test]
fn every_shown_panel_layer_has_a_tab_in_the_panel_order() {
    let board = board();
    let tabs = strip_layers(&board, &PcbLayerView::default());
    assert_eq!(
        tabs[..2],
        [
            PcbLayer::Copper(CopperLayer::Top),
            PcbLayer::Copper(CopperLayer::Bottom),
        ]
    );
    for (top, bottom) in LayerPair::ALL.into_iter().filter_map(LayerPair::layers) {
        assert!(tabs.contains(&top) && tabs.contains(&bottom), "{top:?}");
    }
    for layer in SINGLE_TECHNICAL_LAYERS.map(PcbLayer::Technical) {
        assert!(tabs.contains(&layer), "{layer:?}");
    }
    let drill = PcbLayer::Technical(TechnicalLayer::DrillDrawing);
    assert!(tabs.contains(&drill));
    // The panel's order: a pair's top before its bottom, the pairs before
    // the single layers.
    let at = |layer: PcbLayer| tabs.iter().position(|&l| l == layer);
    let silk_top = at(PcbLayer::Technical(TechnicalLayer::SilkscreenTop));
    let silk_bottom = at(PcbLayer::Technical(TechnicalLayer::SilkscreenBottom));
    assert!(silk_top < silk_bottom);
    assert!(silk_bottom < at(PcbLayer::Technical(TechnicalLayer::Dimensions)));
}

/// Hiding a component pair layer (its eye in the panel) takes its tab
/// away; showing it brings the tab back.
#[test]
fn hiding_a_pair_layer_removes_its_tab_and_showing_it_adds_it() {
    let board = board();
    let mut view = PcbLayerView::default();
    let courtyard = PcbLayer::Technical(TechnicalLayer::CourtyardTop);
    assert!(strip_layers(&board, &view).contains(&courtyard));
    view.toggle_visibility(courtyard);
    assert!(!strip_layers(&board, &view).contains(&courtyard));
    view.toggle_visibility(courtyard);
    assert!(strip_layers(&board, &view).contains(&courtyard));
}
