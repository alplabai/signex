use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::*;
use crate::pcb::{BoardCommand, BoardCommandError, BoardEngine, BoardItemId};

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({"uuid": Uuid::from_u128(1)})).unwrap()
}

fn silk() -> GraphicStyle {
    GraphicStyle::on(PcbLayer::Technical(TechnicalLayer::SilkscreenTop))
}

/// #2424: each drawing is one undo step, and undo takes the board back.
#[test]
fn each_drawing_is_one_undo_step() {
    let mut engine = BoardEngine::new(board(), 1);
    let before = serde_json::to_string(engine.board()).unwrap();
    let o = Point::new(0.0, 0.0);
    let p = Point::new(4.0, 3.0);
    let drawings = [
        new_line(Uuid::from_u128(10), o, p, silk()),
        new_arc(Uuid::from_u128(11), o, Point::new(5.0, 0.0), p, silk()),
        new_circle(Uuid::from_u128(12), o, p, silk()),
        new_rectangle(Uuid::from_u128(13), o, p, silk()),
        new_text_frame(Uuid::from_u128(14), o, p, silk(), "NOTE".into(), 0.0),
    ];
    for drawing in drawings {
        engine
            .execute(place_graphic(drawing.expect("a drawing")))
            .unwrap();
    }
    assert_eq!(engine.board().graphics.len(), 5);
    let kinds: Vec<&str> = engine
        .board()
        .graphics
        .iter()
        .map(|g| g.graphic_type.as_str())
        .collect();
    assert_eq!(kinds, ["line", "arc", "circle", "rect", "text_frame"]);
    for _ in 0..5 {
        engine.undo().unwrap();
    }
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
}

#[test]
fn an_arcs_end_lands_on_its_circle_and_a_circle_takes_its_radius() {
    let o = Point::new(0.0, 0.0);
    let arc = new_arc(
        Uuid::nil(),
        o,
        Point::new(5.0, 0.0),
        Point::new(0.0, 2.0),
        silk(),
    )
    .unwrap();
    assert_eq!(arc.radius, 5.0);
    assert_eq!(arc.end, Some(Point::new(0.0, 5.0)));
    let circle = new_circle(Uuid::nil(), o, Point::new(3.0, 4.0), silk()).unwrap();
    assert_eq!(circle.radius, 5.0);
    let frame = new_text_frame(
        Uuid::nil(),
        o,
        Point::new(2.0, 2.0),
        silk(),
        "A".into(),
        0.0,
    );
    assert_eq!(
        frame.unwrap().font_size,
        signex_types::pcb::PCB_DEFAULT_TEXT_SIZE_MM
    );
}

#[test]
fn copper_keep_out_and_shapes_with_nothing_to_draw_are_refused() {
    let o = Point::new(0.0, 0.0);
    let p = Point::new(4.0, 3.0);
    let copper = GraphicStyle::on(PcbLayer::Copper(CopperLayer::Top));
    let keep_out = GraphicStyle::on(PcbLayer::Technical(TechnicalLayer::KeepoutTop));
    assert!(new_line(Uuid::nil(), o, p, copper).is_none());
    assert!(new_line(Uuid::nil(), o, p, keep_out).is_none());
    assert!(new_line(Uuid::nil(), o, o, silk()).is_none());
    assert!(new_circle(Uuid::nil(), o, o, silk()).is_none());
    assert!(new_arc(Uuid::nil(), o, o, p, silk()).is_none());
    assert!(new_rectangle(Uuid::nil(), o, Point::new(4.0, 0.0), silk()).is_none());
    let nan = Point::new(f64::NAN, 0.0);
    assert!(new_line(Uuid::nil(), o, nan, silk()).is_none());
}

/// A drawing is a board item: delete and replace keep its id and index, and
/// a replace onto copper is refused.
#[test]
fn a_drawing_is_deleted_and_replaced_by_id() {
    let mut engine = BoardEngine::new(board(), 1);
    let id = Uuid::from_u128(20);
    let line = new_line(id, Point::new(0.0, 0.0), Point::new(1.0, 0.0), silk()).unwrap();
    engine.execute(place_graphic(line.clone())).unwrap();
    let mut wider = line.clone();
    wider.width = 0.3;
    engine
        .execute(BoardCommand::ReplaceItems {
            items: vec![BoardItem::Graphic(wider)],
        })
        .unwrap();
    assert_eq!(engine.board().graphics[0].width, 0.3);
    let mut onto_copper = line.clone();
    onto_copper.layer = "TopCopper".into();
    let refused = engine.execute(BoardCommand::ReplaceItems {
        items: vec![BoardItem::Graphic(onto_copper)],
    });
    assert!(matches!(
        refused,
        Err(BoardCommandError::NotGraphicLayer(_))
    ));
    engine
        .execute(BoardCommand::DeleteItems {
            items: vec![BoardItemId::Graphic(id)],
        })
        .unwrap();
    assert!(engine.board().graphics.is_empty());
    engine.undo().unwrap();
    assert_eq!(engine.board().graphics[0].uuid, id);
}
