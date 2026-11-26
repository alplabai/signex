use std::collections::BTreeSet;

use signex_renderer::pcb::{PcbLayerView, PcbSnapshot};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::super::{PcbCanvas, board_without, carried_board};

const R1: u128 = 0x8e98_ba95_1d0f_4d9f_84e0_27dd_f90e_10b5;

/// R1 at (10, 20) with a silkscreen line from local (1, 0) to (2, 0).
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651",
          "footprints": [{
            "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5",
            "reference": "R1", "value": "10k",
            "position": { "x": 10.0, "y": 20.0 }, "rotation": 0.0,
            "layer": "TopCopper",
            "graphics": [
              { "graphic_type": "line", "layer": "TopSilkscreen", "width": 0.12,
                "start": { "x": 1.0, "y": 0.0 }, "end": { "x": 2.0, "y": 0.0 } }
            ]
          }]
        }"#,
    )
    .expect("valid board")
}

/// The drag preview draws a carried footprint's silkscreen at the carry's
/// offset, and the board under it no longer draws it (#2387).
#[test]
fn the_carried_footprints_drawings_move_with_the_drag_preview() {
    let board = board();
    let view = PcbLayerView::default();
    let carried: BTreeSet<Uuid> = [Uuid::from_u128(R1)].into();
    let mut canvas = PcbCanvas::new();
    canvas.set_renderer_snapshot(Some(PcbSnapshot::from_board_with_view(&board, &view)));

    let base = PcbSnapshot::from_board_with_view(&board_without(&board, &carried, &[]), &view);
    assert!(
        base.graphics.is_empty(),
        "the board leaves the carried silk out"
    );
    canvas.begin_drag_preview(carried.clone(), false, Vec::new(), base);
    let delta = Point::new(3.0, 0.0);
    canvas.move_drag_preview(delta, || {
        PcbSnapshot::from_board_with_view(&carried_board(&board, &carried, &[], delta), &view)
    });
    let scene = canvas.drag_preview_scene().expect("a moving scene");
    assert!(
        scene
            .overlay_lines
            .iter()
            .any(|l| l.p0 == [14.0, 20.0] && l.p1 == [15.0, 20.0]),
        "{:?}",
        scene.overlay_lines
    );
}
