//! #2507: each dimension kind placed as one undo step, and a radial
//! dimension following the arc or circle it measures in the edit's step.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_dimension_kind::DimensionKind;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::draw::{new_arc, new_kind_dimension, new_radial_dimension, place_item};
use super::items::BoardItem;
use super::{BoardCommand, BoardEngine};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

fn overlay() -> PcbLayer {
    PcbLayer::Technical(TechnicalLayer::SilkscreenTop)
}

/// A board with one free arc about (7, 0) of radius 2 on the top overlay.
fn board_with_arc() -> (BoardEngine, Uuid) {
    let board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::from_u128(1) })).unwrap();
    let mut engine = BoardEngine::new(board, 1);
    let id = Uuid::from_u128(0x20);
    let points = (p(5.0, 0.0), p(7.0, 2.0), p(9.0, 0.0));
    let arc = new_arc(engine.board(), id, points, overlay(), 0.2).unwrap();
    engine.execute(place_item(BoardItem::Arc(arc))).unwrap();
    (engine, id)
}

#[test]
fn a_horizontal_and_a_vertical_dimension_measure_across_and_up() {
    let id = Uuid::from_u128(0x40);
    let clicks = (p(0.0, 0.0), p(10.0, 4.0), p(3.0, 7.0));
    let across = new_kind_dimension(DimensionKind::Horizontal, id, clicks, overlay()).unwrap();
    assert_eq!(across.kind, DimensionKind::Horizontal);
    assert_eq!(across.value_mm(), 10.0);
    assert_eq!(across.offset, 7.0, "the third click stands 7 mm up");
    let up = new_kind_dimension(DimensionKind::Vertical, id, clicks, overlay()).unwrap();
    assert_eq!(up.value_mm(), 4.0);
    assert_eq!(up.offset, -3.0, "3 mm right of a line running up");
    let aligned = new_kind_dimension(DimensionKind::Aligned, id, clicks, overlay()).unwrap();
    assert_eq!(aligned.kind, DimensionKind::Aligned);
    assert!((aligned.value_mm() - 116f64.sqrt()).abs() < 1e-12);
    let level = (p(0.0, 0.0), p(0.0, 5.0), p(3.0, 0.0));
    assert!(
        new_kind_dimension(DimensionKind::Horizontal, id, level, overlay()).is_none(),
        "nothing across"
    );
    assert!(new_kind_dimension(DimensionKind::Radial, id, clicks, overlay()).is_none());
}

#[test]
fn a_radial_dimension_lands_on_the_arc_nearest_the_click_in_one_step() {
    let (mut engine, arc) = board_with_arc();
    let id = Uuid::from_u128(0x41);
    assert!(
        new_radial_dimension(engine.board(), id, p(30.0, 30.0), overlay()).is_none(),
        "no arc there"
    );
    let d = new_radial_dimension(engine.board(), id, p(7.0, 2.5), overlay()).unwrap();
    assert_eq!((d.kind, d.measures), (DimensionKind::Radial, Some(arc)));
    assert!(close(d.start, p(7.0, 0.0)) && close(d.end, p(7.0, 2.0)));
    assert!((d.value_mm() - 2.0).abs() < 1e-9);
    assert_eq!(d.value_text(), "R2.00mm");
    engine.execute(place_item(BoardItem::Dimension(d))).unwrap();
    assert_eq!(engine.board().free.dimensions.len(), 1);
    engine.undo().unwrap();
    assert!(engine.board().free.dimensions.is_empty());
}

#[test]
fn a_radial_dimension_follows_its_arc_in_the_same_undo_step() {
    let (mut engine, arc) = board_with_arc();
    let id = Uuid::from_u128(0x41);
    let d = new_radial_dimension(engine.board(), id, p(7.0, 2.5), overlay()).unwrap();
    engine.execute(place_item(BoardItem::Dimension(d))).unwrap();
    // The arc moves 10 mm right and grows to radius 3.
    let mut moved = engine.board().free.arcs[0].clone();
    (moved.start, moved.mid, moved.end) = (p(14.0, 0.0), p(17.0, 3.0), p(20.0, 0.0));
    let steps = engine.undo_len();
    engine
        .execute(BoardCommand::ReplaceItems {
            items: vec![BoardItem::Arc(moved)],
        })
        .unwrap();
    assert_eq!(engine.undo_len(), steps + 1, "one step for both");
    let followed = &engine.board().free.dimensions[0];
    assert_eq!(followed.measures, Some(arc));
    assert!(close(followed.start, p(17.0, 0.0)), "{:?}", followed.start);
    assert!(close(followed.end, p(17.0, 3.0)), "{:?}", followed.end);
    assert_eq!(followed.value_text(), "R3.00mm");
    engine.undo().unwrap();
    let back = &engine.board().free.dimensions[0];
    assert!(close(back.start, p(7.0, 0.0)) && close(back.end, p(7.0, 2.0)));
    engine.redo().unwrap();
    assert!(close(engine.board().free.dimensions[0].end, p(17.0, 3.0)));
}

/// A drawn circle is measured by its rim point, as the view and the
/// outputs draw it (DECISIONS 2026-11-25), and a radial dimension on it
/// follows it when it moves.
#[test]
fn a_drawn_circle_is_measured_by_its_rim_and_followed() {
    let circle = Uuid::from_u128(0x30);
    let board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "graphics": [{"uuid": circle, "graphic_type": "circle", "layer": "TopSilkscreen",
                      "width": 0.2, "start": null, "end": {"x": 25.0, "y": 20.0},
                      "center": {"x": 20.0, "y": 20.0}, "radius": 4.0}]
    }))
    .unwrap();
    let found = super::dimension_refit::circle_by_id(&board, circle).unwrap();
    assert_eq!(found.radius, 5.0, "the rim, not the saved radius");
    let mut engine = BoardEngine::new(board, 1);
    let id = Uuid::from_u128(0x41);
    let d = new_radial_dimension(engine.board(), id, p(20.0, 25.5), overlay()).unwrap();
    assert_eq!(d.measures, Some(circle));
    engine.execute(place_item(BoardItem::Dimension(d))).unwrap();
    let mut moved = engine.board().graphics[0].clone();
    moved.center = Some(p(0.0, 20.0));
    moved.end = Some(p(5.0, 20.0));
    engine
        .execute(BoardCommand::ReplaceItems {
            items: vec![BoardItem::Graphic(moved)],
        })
        .unwrap();
    let followed = &engine.board().free.dimensions[0];
    assert!(close(followed.start, p(0.0, 20.0)), "{:?}", followed.start);
    assert!(close(followed.end, p(0.0, 25.0)), "{:?}", followed.end);
}

#[test]
fn an_edit_that_leaves_the_arc_where_it_is_does_not_touch_the_dimension() {
    let (mut engine, _) = board_with_arc();
    let id = Uuid::from_u128(0x41);
    let d = new_radial_dimension(engine.board(), id, p(7.0, 2.5), overlay()).unwrap();
    let patches = engine
        .execute_batch(vec![place_item(BoardItem::Dimension(d.clone()))])
        .unwrap();
    assert_eq!(patches.len(), 1, "no refit owed");
    // Moving the dimension alone puts it back on its arc.
    let mut off = d.clone();
    off.start = p(1.0, 1.0);
    off.end = p(1.0, 5.0);
    engine
        .execute(BoardCommand::ReplaceItems {
            items: vec![BoardItem::Dimension(off)],
        })
        .unwrap();
    let back = &engine.board().free.dimensions[0];
    assert!(close(back.start, p(7.0, 0.0)) && close(back.end, p(7.0, 2.0)));
}
