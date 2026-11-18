use super::*;
use crate::pcb::{PcbLayerView, PcbSnapshot};
use signex_types::pcb::{PCB_DEFAULT_TEXT_SIZE_MM, PcbBoard, Point};
use signex_types::pcb_layer::TechnicalLayer;

const TOP_OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const BOTTOM_OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenBottom);
const TOP_FAB: PcbLayer = PcbLayer::Technical(TechnicalLayer::FabTop);

/// R1 at (10, 20), turned 90 degrees, its designator 1 mm along its local
/// x axis on the top overlay, its comment at the default spot on Top Fab.
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651",
          "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" }
          ],
          "footprints": [{
            "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5",
            "reference": "R1", "value": "10k",
            "position": { "x": 10.0, "y": 20.0 }, "rotation": 90.0,
            "layer": "TopCopper",
            "properties": [
              { "key": "Reference", "value": "R1", "position": { "x": 1.0, "y": 0.0 },
                "rotation": 0.0, "layer": "TopSilkscreen", "font_size": 0.8 },
              { "key": "Value", "value": "10k", "layer": "TopFab" },
              { "key": "MPN", "value": "RC0603", "layer": "TopFab" }
            ]
          }]
        }"#,
    )
    .expect("valid board")
}

fn texts(board: &PcbBoard, view: &PcbLayerView) -> Vec<FootprintTextInput> {
    PcbSnapshot::from_board_with_view(board, view).texts
}

fn designator(texts: &[FootprintTextInput]) -> Option<&FootprintTextInput> {
    texts.iter().find(|t| t.content == "R1")
}

#[test]
fn the_designator_is_at_its_place_on_its_overlay_layer() {
    let shown = texts(&board(), &PcbLayerView::default());
    let r1 = designator(&shown).expect("the designator is drawn");
    // Local (1, 0) turned a quarter: (0, 1) from the origin.
    assert_eq!(r1.position, [10.0, 21.0]);
    assert_eq!(r1.rotation_deg, 90.0);
    assert_eq!(r1.size_mm, 0.8);
    assert_eq!(r1.layer, TOP_OVERLAY);
    assert!(!r1.mirrored);
    let comment = shown.iter().find(|t| t.content == "10k").expect("comment");
    assert_eq!(comment.layer, TOP_FAB);
    assert_eq!(comment.size_mm, PCB_DEFAULT_TEXT_SIZE_MM as f32);
}

#[test]
fn a_hidden_text_is_not_drawn() {
    let mut board = board();
    assert!(
        !texts(&board, &PcbLayerView::default())
            .iter()
            .any(|t| t.content == "RC0603"),
        "a parameter is not a board text"
    );
    board.footprints[0].properties[0].hidden = true;
    assert!(designator(&texts(&board, &PcbLayerView::default())).is_none());
}

#[test]
fn the_text_hides_with_its_layer() {
    let board = board();
    let mut view = PcbLayerView::default();
    view.toggle_visibility(TOP_OVERLAY);
    let shown = texts(&board, &view);
    assert!(designator(&shown).is_none(), "its layer is hidden");
    assert!(shown.iter().any(|t| t.content == "10k"), "Top Fab is shown");
    view.toggle_visibility(TOP_OVERLAY);
    assert!(designator(&texts(&board, &view)).is_some());
}

#[test]
fn single_layer_mode_draws_only_the_active_layers_text() {
    let board = board();
    let mut view = PcbLayerView::default();
    view.toggle_single_layer();
    assert!(texts(&board, &view).is_empty(), "Top Copper is active");
    view.set_active(TOP_OVERLAY);
    let shown = texts(&board, &view);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].content, "R1");
}

#[test]
fn a_flip_mirrors_the_text_onto_the_bottom_overlay() {
    let mut board = board();
    pcb_geom::flip_footprint(&mut board.footprints[0]);
    let shown = texts(&board, &PcbLayerView::default());
    let r1 = designator(&shown).expect("the designator is drawn");
    assert_eq!(r1.layer, BOTTOM_OVERLAY);
    assert!(r1.mirrored);
    // Mirrored x (-1, 0), turned -90 (270) degrees: (0, 1) from the origin,
    // the same spot a physical flip about the vertical axis leaves it.
    assert_eq!(r1.position, [10.0, 21.0]);
    assert_eq!(r1.rotation_deg, 270.0);
}

#[test]
fn a_text_with_no_layer_takes_its_sides_overlay() {
    let mut board = board();
    board.footprints[0].properties[0].layer = None;
    assert_eq!(
        designator(&texts(&board, &PcbLayerView::default())).map(|t| t.layer),
        Some(TOP_OVERLAY)
    );
    board.footprints[0].layer = "BottomCopper".into();
    assert_eq!(
        designator(&texts(&board, &PcbLayerView::default())).map(|t| t.layer),
        Some(BOTTOM_OVERLAY)
    );
}

#[test]
fn the_text_moves_with_its_footprint() {
    let mut board = board();
    board.footprints[0].position = Point::new(30.0, 5.0);
    let shown = texts(&board, &PcbLayerView::default());
    assert_eq!(designator(&shown).map(|t| t.position), Some([30.0, 6.0]));
}

#[test]
fn the_text_keeps_its_justification() {
    let mut board = board();
    assert_eq!(
        designator(&texts(&board, &PcbLayerView::default())).map(|t| t.justify),
        Some(HAlign::Center)
    );
    board.footprints[0].properties[0].justify = HAlign::Left;
    assert_eq!(
        designator(&texts(&board, &PcbLayerView::default())).map(|t| t.justify),
        Some(HAlign::Left)
    );
}
