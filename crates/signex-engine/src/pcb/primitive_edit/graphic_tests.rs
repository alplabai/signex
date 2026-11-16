use signex_types::pcb::{BoardGraphic, PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::*;
use crate::pcb::graphic::{GraphicStyle, new_circle, new_line, new_text_frame, place_graphic};
use crate::pcb::{BoardCommand, BoardEngine, BoardItem};

const LINE: u128 = 30;
const CIRCLE: u128 = 31;
const FRAME: u128 = 32;

fn board() -> PcbBoard {
    let silk = GraphicStyle::on(PcbLayer::Technical(TechnicalLayer::SilkscreenTop));
    let o = Point::new(0.0, 0.0);
    let p = Point::new(4.0, 3.0);
    let mut engine = BoardEngine::new(
        serde_json::from_value(serde_json::json!({"uuid": Uuid::from_u128(1)})).unwrap(),
        1,
    );
    for drawing in [
        new_line(Uuid::from_u128(LINE), o, p, silk),
        new_circle(Uuid::from_u128(CIRCLE), o, p, silk),
        new_text_frame(Uuid::from_u128(FRAME), o, p, silk, "A".into(), 1.0),
    ] {
        engine.execute(place_graphic(drawing.unwrap())).unwrap();
    }
    engine.board().clone()
}

fn edited(board: &PcbBoard, id: u128, edit: GraphicEdit) -> BoardGraphic {
    let Some(BoardCommand::ReplaceItems { items }) =
        graphic_edit_command(board, &[Uuid::from_u128(id)], &edit).unwrap()
    else {
        panic!("a replace");
    };
    let BoardItem::Graphic(g) = items[0].clone() else {
        panic!("a drawing");
    };
    g
}

/// #2424: the Properties fields of a drawing, one replace each.
#[test]
fn each_field_edits_the_drawings_that_have_it() {
    let b = board();
    let fab = PcbLayer::Technical(TechnicalLayer::FabTop);
    assert_eq!(edited(&b, LINE, GraphicEdit::Layer(fab)).layer, "TopFab");
    assert_eq!(edited(&b, LINE, GraphicEdit::Width(0.3)).width, 0.3);
    assert_eq!(
        edited(&b, LINE, GraphicEdit::EndX(9.0)).end,
        Some(Point::new(9.0, 3.0))
    );
    let bigger = edited(&b, CIRCLE, GraphicEdit::Radius(10.0));
    assert_eq!(bigger.radius, 10.0);
    assert_eq!(bigger.end, Some(Point::new(8.0, 6.0)));
    let moved = edited(&b, CIRCLE, GraphicEdit::CenterX(2.0));
    assert_eq!(moved.center, Some(Point::new(2.0, 0.0)));
    assert_eq!(moved.end, Some(Point::new(6.0, 3.0)));
    assert_eq!(
        edited(&b, FRAME, GraphicEdit::Text("A\nB".into())).text,
        "A\nB"
    );
    assert_eq!(
        edited(&b, FRAME, GraphicEdit::TextHeight(2.0)).font_size,
        2.0
    );
}

#[test]
fn a_field_a_drawing_lacks_or_an_odd_value_changes_nothing() {
    let b = board();
    let line = [Uuid::from_u128(LINE)];
    assert_eq!(
        graphic_edit_command(&b, &line, &GraphicEdit::Radius(3.0)),
        Ok(None)
    );
    assert_eq!(
        graphic_edit_command(&b, &line, &GraphicEdit::Width(-1.0)),
        Err(PrimitiveEditRefusal::Negative)
    );
    assert_eq!(
        graphic_edit_command(
            &b,
            &line,
            &GraphicEdit::Layer(PcbLayer::Copper(CopperLayer::Top))
        ),
        Err(PrimitiveEditRefusal::GraphicLayerNotAllowed)
    );
    let circle = [Uuid::from_u128(CIRCLE)];
    assert_eq!(
        graphic_edit_command(&b, &circle, &GraphicEdit::Radius(0.0)),
        Err(PrimitiveEditRefusal::NotPositive)
    );
    assert_eq!(
        graphic_edit_command(&b, &[Uuid::from_u128(99)], &GraphicEdit::Width(1.0)),
        Err(PrimitiveEditRefusal::Gone)
    );
}
