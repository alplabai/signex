use signex_types::pcb::{Footprint, PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

/// A two-pad diode footprint in its top-side library form.
fn template(layer: &str) -> Footprint {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xf0), "footprint_id": "Chip-0805-Polarised",
        "position": {"x": 0.0, "y": 0.0}, "rotation": 0.0, "layer": layer,
        "pads": [
            {"uuid": Uuid::from_u128(0xf1), "number": "1", "pad_type": "smd",
             "shape": "rect", "position": {"x": -1.0, "y": 0.0},
             "size": {"x": 1.0, "y": 1.2}, "drill": null, "layers": ["TopCopper"]},
            {"uuid": Uuid::from_u128(0xf2), "number": "2", "pad_type": "smd",
             "shape": "rect", "position": {"x": 1.0, "y": 0.0},
             "size": {"x": 1.0, "y": 1.2}, "drill": null, "layers": ["TopCopper"]}
        ],
        "properties": [{"key": "Reference", "value": ""}, {"key": "Value", "value": ""}]
    }))
    .unwrap()
}

fn board_with_d3() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [{"uuid": Uuid::from_u128(0x10), "reference": "D3",
                        "position": {"x": 0.0, "y": 0.0}, "layer": "TopCopper"}]
    }))
    .unwrap()
}

#[test]
fn a_click_adds_the_footprint_at_the_point_with_the_next_designator_in_one_step() {
    let part = PartToPlace::new(template("TopCopper"), "D", "1N4148").unwrap();
    let mut engine = BoardEngine::new(board_with_d3(), 1);
    let at = Point::new(12.5, 7.5);
    let first = Uuid::from_u128(0x100);
    engine
        .execute_batch(vec![part.place_command(engine.board(), first, at)])
        .unwrap();
    let second = Uuid::from_u128(0x200);
    engine
        .execute_batch(vec![part.place_command(engine.board(), second, at)])
        .unwrap();

    let placed = |id| engine.board().footprints.iter().find(|f| f.uuid == id);
    let d4 = placed(first).expect("first copy");
    assert_eq!((d4.reference.as_str(), d4.value.as_str()), ("D4", "1N4148"));
    assert_eq!(d4.position, at);
    assert_eq!(d4.footprint_id, "Chip-0805-Polarised");
    assert_eq!(d4.pads.len(), 2);
    assert_eq!(d4.properties[0].value, "D4");
    assert_eq!(d4.properties[1].value, "1N4148");
    assert_eq!(placed(second).unwrap().reference, "D5");
    // Each copy's pads are its own.
    let pads = |id| {
        placed(id)
            .unwrap()
            .pads
            .iter()
            .map(|p| p.uuid)
            .collect::<Vec<_>>()
    };
    assert!(pads(first).iter().all(|p| !pads(second).contains(p)));

    engine.undo().unwrap();
    let on_board = |id| engine.board().footprints.iter().any(|f| f.uuid == id);
    assert!(!on_board(second) && on_board(first), "one undo per click");
}

#[test]
fn turns_and_the_flip_shape_what_lands() {
    let mut part = PartToPlace::new(template("TopCopper"), "D", "").unwrap();
    part.rotate(1);
    let fp = part.footprint_at(Uuid::from_u128(9), Point::new(0.0, 0.0), "D1");
    assert_eq!((fp.rotation, fp.layer.as_str()), (90.0, "TopCopper"));
    part.rotate(-2);
    part.flip();
    assert!(part.is_bottom());
    let fp = part.footprint_at(Uuid::from_u128(9), Point::new(0.0, 0.0), "D1");
    // A flip mirrors the turn: 270° on top reads 90° on the bottom.
    assert_eq!((fp.rotation, fp.layer.as_str()), (90.0, "BottomCopper"));
    assert!(fp.pads.iter().all(|p| p.layers == ["BottomCopper"]));
}

#[test]
fn a_template_off_the_top_side_is_refused() {
    let refused = PartToPlace::new(template("BottomCopper"), "D", "");
    assert_eq!(
        refused,
        Err(BoardCommandError::TemplateNotTopSide(Uuid::from_u128(0xf0)))
    );
    assert!(PartToPlace::new(template("NotALayer"), "D", "").is_err());
}

#[test]
fn the_preview_holds_only_the_part() {
    let part = PartToPlace::new(template("TopCopper"), "D", "").unwrap();
    let preview = part.preview_board(&board_with_d3(), Point::new(3.0, 4.0));
    assert_eq!(preview.footprints.len(), 1);
    assert_eq!(preview.footprints[0].reference, "D4");
    assert_eq!(preview.footprints[0].position, Point::new(3.0, 4.0));
}
