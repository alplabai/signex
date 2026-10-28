use crate::test_support::test_sheet;
use crate::*;
use signex_types::schematic::{DrawingPlacement, Point, SchDrawing};
use uuid::Uuid;

fn line(n: u128) -> SchDrawing {
    SchDrawing::Line {
        uuid: Uuid::from_u128(n),
        start: Point::new(n as f64, 0.0),
        end: Point::new(n as f64, 5.0),
        width: 0.25,
        stroke_color: None,
    }
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// An engine over a sheet whose drawings are 1..=`count` in that order.
fn engine_with(count: u128) -> Engine {
    let mut sheet = test_sheet();
    sheet.drawings = (1..=count).map(line).collect();
    Engine::new(sheet).unwrap()
}

fn order(engine: &Engine) -> Vec<u128> {
    engine
        .document()
        .drawings
        .iter()
        .map(|d| d.uuid().as_u128())
        .collect()
}

fn arrange(engine: &mut Engine, ids: &[u128], how: DrawingArrange) -> CommandResult {
    engine
        .execute(Command::ArrangeDrawings {
            ids: ids.iter().copied().map(id).collect(),
            arrange: how,
        })
        .unwrap()
}

#[test]
fn to_front_and_to_back_move_to_the_ends() {
    let mut engine = engine_with(5);
    arrange(&mut engine, &[2], DrawingArrange::ToFront);
    assert_eq!(order(&engine), [1, 3, 4, 5, 2]);

    arrange(&mut engine, &[4], DrawingArrange::ToBack);
    assert_eq!(order(&engine), [4, 1, 3, 5, 2]);
}

/// A multi-selection keeps the order it had in the list, whatever order the
/// ids arrive in.
#[test]
fn a_multi_selection_keeps_its_relative_order() {
    let mut engine = engine_with(6);
    arrange(&mut engine, &[5, 2, 4], DrawingArrange::ToFront);
    assert_eq!(order(&engine), [1, 3, 6, 2, 4, 5]);

    arrange(&mut engine, &[6, 3], DrawingArrange::ToBack);
    assert_eq!(order(&engine), [3, 6, 1, 2, 4, 5]);
}

#[test]
fn forward_and_backward_step_one_unselected_neighbour() {
    let mut engine = engine_with(5);
    arrange(&mut engine, &[2], DrawingArrange::Forward);
    assert_eq!(order(&engine), [1, 3, 2, 4, 5]);

    arrange(&mut engine, &[2], DrawingArrange::Backward);
    assert_eq!(order(&engine), [1, 2, 3, 4, 5]);

    // A block moves together and keeps its shape.
    arrange(&mut engine, &[2, 3], DrawingArrange::Forward);
    assert_eq!(order(&engine), [1, 4, 2, 3, 5]);
    arrange(&mut engine, &[2, 5], DrawingArrange::Backward);
    assert_eq!(order(&engine), [1, 2, 4, 5, 3]);
}

#[test]
fn a_selection_already_at_the_end_changes_nothing_and_records_nothing() {
    let mut engine = engine_with(3);
    assert!(!arrange(&mut engine, &[3], DrawingArrange::ToFront).changed);
    assert!(!arrange(&mut engine, &[1], DrawingArrange::Backward).changed);
    assert!(!arrange(&mut engine, &[2, 3], DrawingArrange::Forward).changed);
    assert!(engine.undo().unwrap().is_none(), "nothing was recorded");
}

#[test]
fn unknown_ids_are_ignored() {
    let mut engine = engine_with(3);
    assert!(!arrange(&mut engine, &[99], DrawingArrange::ToFront).changed);
    arrange(&mut engine, &[99, 1], DrawingArrange::ToFront);
    assert_eq!(order(&engine), [2, 3, 1]);
}

#[test]
fn an_arrange_is_one_undoable_step() {
    let mut engine = engine_with(5);
    arrange(&mut engine, &[1, 3], DrawingArrange::ToFront);
    assert_eq!(order(&engine), [2, 4, 5, 1, 3]);

    engine.undo().unwrap().expect("undo");
    assert_eq!(order(&engine), [1, 2, 3, 4, 5]);
    engine.redo().unwrap().expect("redo");
    assert_eq!(order(&engine), [2, 4, 5, 1, 3]);
}

#[test]
fn a_placement_is_set_cleared_and_undoable() {
    let mut engine = engine_with(3);
    let set = |engine: &mut Engine, ids: &[u128], placement| {
        engine
            .execute(Command::SetDrawingPlacement {
                ids: ids.iter().copied().map(id).collect(),
                placement,
            })
            .unwrap()
    };

    assert!(set(&mut engine, &[1, 2], DrawingPlacement::Above).changed);
    assert_eq!(
        engine.document().drawing_placement(id(1)),
        DrawingPlacement::Above
    );
    assert_eq!(
        engine.document().drawing_placement(id(3)),
        DrawingPlacement::Behind
    );
    assert!(!set(&mut engine, &[1, 2], DrawingPlacement::Above).changed);

    // Back to behind removes the entry: the sheet is as it was.
    assert!(set(&mut engine, &[1, 2], DrawingPlacement::Behind).changed);
    assert!(engine.document().drawing_placements.is_empty());

    engine.undo().unwrap().expect("undo");
    assert_eq!(
        engine.document().drawing_placement(id(2)),
        DrawingPlacement::Above
    );
}

#[test]
fn a_placement_for_an_unknown_drawing_is_ignored() {
    let mut engine = engine_with(2);
    let result = engine
        .execute(Command::SetDrawingPlacement {
            ids: vec![id(77)],
            placement: DrawingPlacement::Above,
        })
        .unwrap();
    assert!(!result.changed);
    assert!(engine.document().drawing_placements.is_empty());
}
