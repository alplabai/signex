//! #2868: each look field of a dimension is one replace, one undo step.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_dimension_style::{
    ArrowStyle, BoardFont, DimensionTextPosition, DimensionUnits,
};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardCommand;

const ID: Uuid = Uuid::from_u128(4);

fn board() -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::from_u128(1) })).unwrap();
    board.free.dimensions.push(LinearDimension::new(
        ID,
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
        2.0,
    ));
    board
}

fn edit(
    board: &PcbBoard,
    e: DimensionStyleEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    dimension_edit_command(board, &[ID], DimensionEdit::Style(e))
}

#[test]
fn every_look_field_is_one_replace_of_the_dimension() {
    use DimensionStyleEdit as E;
    let edits = [
        E::ArrowStyle(ArrowStyle::Filled),
        E::ArrowLength(2.0),
        E::ArrowWidth(0.4),
        E::Font(BoardFont::Sans),
        E::Bold(true),
        E::Italic(true),
        E::TextWidth(0.8),
        E::TextPosition(DimensionTextPosition::Inline),
        E::TextGap(0.25),
        E::ExtensionOffset(0.5),
        E::ExtensionOvershoot(1.0),
        E::ExtensionWidth(0.2),
        E::Unit(DimensionUnits::Mil),
        E::ShowUnit(false),
        E::Prefix("W ".into()),
        E::Suffix(" max".into()),
        E::TrailingZeros(false),
    ];
    for e in edits {
        let mut b = board();
        let before = b.free.dimensions[0].clone();
        let command = edit(&b, e.clone()).expect("taken").expect("a change");
        let BoardCommand::ReplaceItems { items } = &command else {
            panic!("{e:?} is not one replace: {command:?}");
        };
        assert_eq!(items.len(), 1, "{e:?}");
        command.apply(&mut b).expect("applies");
        assert_ne!(b.free.dimensions[0], before, "{e:?} changed nothing");
        assert_eq!(edit(&b, e.clone()), Ok(None), "{e:?} again is no step");
    }
}

#[test]
fn picking_manual_keeps_the_value_where_it_is_drawn() {
    let mut b = board();
    let drawn = b.free.dimensions[0].drawing().unwrap().label_at;
    let manual = DimensionStyleEdit::TextPosition(DimensionTextPosition::Manual);
    edit(&b, manual).unwrap().unwrap().apply(&mut b).unwrap();
    let d = &b.free.dimensions[0];
    assert_eq!(d.style.text_position, DimensionTextPosition::Manual);
    let now = d.drawing().unwrap().label_at;
    assert!((now.x - drawn.x).abs() < 1e-9 && (now.y - drawn.y).abs() < 1e-9);
    let along = DimensionStyleEdit::TextAlong(3.0);
    edit(&b, along).unwrap().unwrap().apply(&mut b).unwrap();
    let moved = b.free.dimensions[0].drawing().unwrap().label_at;
    assert!((moved.x - (drawn.x + 3.0)).abs() < 1e-9);
}

#[test]
fn a_negative_or_wild_look_length_is_refused() {
    let b = board();
    assert_eq!(
        edit(&b, DimensionStyleEdit::ArrowLength(-1.0)),
        Err(PrimitiveEditRefusal::Negative)
    );
    assert_eq!(
        edit(&b, DimensionStyleEdit::TextWidth(0.0)),
        Err(PrimitiveEditRefusal::NotPositive)
    );
    assert_eq!(
        edit(&b, DimensionStyleEdit::ExtensionWidth(f64::NAN)),
        Err(PrimitiveEditRefusal::NonFinite)
    );
}

/// A replace sent past the Properties (an agent or a replayed log) is
/// checked by the engine as well.
#[test]
fn the_engine_refuses_a_replaced_dimension_with_a_negative_look() {
    let mut b = board();
    let mut bad = b.free.dimensions[0].clone();
    bad.style.arrow_width = -0.5;
    let command = BoardCommand::ReplaceItems {
        items: vec![crate::pcb::BoardItem::Dimension(bad)],
    };
    assert!(command.apply(&mut b).is_err());
}
