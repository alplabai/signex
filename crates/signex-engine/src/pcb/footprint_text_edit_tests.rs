use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// U1 on top turned 30 degrees, U2 on the bottom turned 90, U3 locked, U4
/// without texts. U1, U2 and U3 carry a designator at the default spot.
fn board() -> PcbBoard {
    let footprint = |n: u128, layer: &str, rotation: f64, locked: bool, texts: bool| {
        let properties = if texts {
            serde_json::json!([
                { "key": "Reference", "value": format!("U{n}") },
                { "key": "Value", "value": "10k", "position": {"x": 0.0, "y": 2.0} }
            ])
        } else {
            serde_json::json!([])
        };
        serde_json::json!({
            "uuid": id(n), "reference": format!("U{n}"),
            "position": {"x": 10.0 * n as f64, "y": 10.0},
            "rotation": rotation, "layer": layer, "locked": locked,
            "properties": properties
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(100),
        "footprints": [
            footprint(1, "TopCopper", 30.0, false, true),
            footprint(2, "BottomCopper", 90.0, false, true),
            footprint(3, "TopCopper", 0.0, true, true),
            footprint(4, "TopCopper", 0.0, false, false)
        ]
    }))
    .unwrap()
}

fn anchor(board: &PcbBoard, n: u128, field: BoardTextField) -> Point {
    let fp = board.footprints.iter().find(|f| f.uuid == id(n)).unwrap();
    pcb_geom::footprint_text(fp, field)
        .unwrap()
        .unwrap()
        .placement
        .anchor
        .to_mm()
}

/// Within a few nm: a board point goes through the integer-nm frame twice.
fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-5 && (a.y - b.y).abs() < 1e-5
}

#[test]
fn a_moved_text_lands_by_the_offset_on_either_side_in_one_step() {
    for n in [1, 2] {
        let board = board();
        let before = anchor(&board, n, BoardTextField::Designator);
        let edit = FootprintTextEdit::MoveBy(Point::new(1.5, -0.5));
        let command = footprint_text_command(&board, id(n), BoardTextField::Designator, edit)
            .expect("allowed")
            .expect("a change");
        let mut engine = BoardEngine::new(board, 1);
        engine.execute_batch(vec![command]).expect("runs");
        let after = anchor(engine.board(), n, BoardTextField::Designator);
        assert!(
            close(after, Point::new(before.x + 1.5, before.y - 0.5)),
            "U{n}: {after:?}"
        );
        // The comment and the footprint stay where they were.
        let fp = &engine.board().footprints[(n - 1) as usize];
        assert_eq!(fp.position, Point::new(10.0 * n as f64, 10.0));
        assert!(engine.undo().is_ok());
        assert!(close(
            anchor(engine.board(), n, BoardTextField::Designator),
            before
        ));
    }
}

#[test]
fn a_text_turned_to_a_board_angle_reads_at_that_angle() {
    for n in [1, 2] {
        let board = board();
        let edit = FootprintTextEdit::Rotation(45.0);
        let command = footprint_text_command(&board, id(n), BoardTextField::Comment, edit)
            .unwrap()
            .unwrap();
        let mut engine = BoardEngine::new(board, 1);
        engine.execute_batch(vec![command]).unwrap();
        let fp = &engine.board().footprints[(n - 1) as usize];
        let text = pcb_geom::footprint_text(fp, BoardTextField::Comment)
            .unwrap()
            .unwrap();
        let off = (text.placement.rotation_deg - 45.0).rem_euclid(360.0);
        assert!(off.min(360.0 - off) < 1e-9, "U{n}: {off}");
    }
}

#[test]
fn height_and_justification_are_set_and_a_bad_height_refused() {
    let board = board();
    let field = BoardTextField::Designator;
    let set = |edit| footprint_text_command(&board, id(1), field, edit);
    match set(FootprintTextEdit::Height(1.5)) {
        Ok(Some(BoardCommand::SetFootprintProperty { property, .. })) => {
            assert_eq!(property.font_size, Some(1.5));
        }
        other => panic!("{other:?}"),
    }
    match set(FootprintTextEdit::Justify(HAlign::Right)) {
        Ok(Some(BoardCommand::SetFootprintProperty { property, .. })) => {
            assert_eq!(property.justify, HAlign::Right);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        set(FootprintTextEdit::Height(0.0)),
        Err(FootprintTextRefusal::BadHeight)
    ));
    assert!(matches!(
        set(FootprintTextEdit::Justify(HAlign::Center)),
        Ok(None)
    ));
    assert!(matches!(
        set(FootprintTextEdit::MoveBy(Point::new(0.0, 0.0))),
        Ok(None)
    ));
}

#[test]
fn a_locked_footprint_or_a_missing_text_refuses() {
    let board = board();
    let edit = FootprintTextEdit::MoveBy(Point::new(1.0, 0.0));
    assert!(matches!(
        footprint_text_command(&board, id(3), BoardTextField::Designator, edit),
        Err(FootprintTextRefusal::Placement(PlacementRefusal::Locked(_)))
    ));
    assert!(matches!(
        footprint_text_command(&board, id(4), BoardTextField::Designator, edit),
        Err(FootprintTextRefusal::Missing { .. })
    ));
}
