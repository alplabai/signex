//! Caller-minted ids (#2429): a command list run twice from the same sheet
//! and the same `SequentialIds` gives the same document, ids included, and a
//! new id already in the sheet is refused.

use signex_types::object_id::{IdSource, SequentialIds};
use signex_types::schematic::{Point, SchematicSheet, TextNote, Wire};
use uuid::Uuid;

use crate::test_support::test_sheet;
use crate::{
    Command, Engine, EngineError, PlacementPayload, child_sheet_from_corners, new_sheet_entry,
};

fn payload() -> PlacementPayload {
    let mut fragment = SchematicSheet::default();
    fragment.wires.push(Wire {
        uuid: Uuid::from_u128(0xA1),
        start: Point::new(0.0, 0.0),
        end: Point::new(20.0, 0.0),
        ..Default::default()
    });
    fragment.text_notes.push(TextNote {
        uuid: Uuid::from_u128(0xA2),
        text: "note".into(),
        position: Point::new(0.0, 5.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: Default::default(),
        justify_v: Default::default(),
        locked: false,
    });
    PlacementPayload::new(fragment)
}

/// Paste, break the pasted wire, place a sheet symbol and an entry on it:
/// four undo steps, every new id from `ids`.
fn run_steps(start: &SchematicSheet, ids: &mut SequentialIds) -> Engine {
    let mut engine = Engine::new(start.clone()).unwrap();
    let commands = payload()
        .place_commands(Point::new(10.0, 10.0), ids)
        .unwrap();
    engine.execute_batch(commands).unwrap();
    let wire = engine.document().wires[0].uuid;
    let new_wire = ids.next_id();
    let at = Point::new(20.0, 10.0);
    engine
        .execute(Command::BreakWire { wire, at, new_wire })
        .unwrap();
    let corners = (Point::new(50.0, 50.0), Point::new(80.0, 80.0));
    let child_sheet = child_sheet_from_corners(corners.0, corners.1, "sheet1", ids).unwrap();
    let sheet_id = child_sheet.uuid;
    let pin = new_sheet_entry(&child_sheet, Point::new(48.0, 60.0), "P1", None, ids);
    engine
        .execute(Command::PlaceChildSheet { child_sheet })
        .unwrap();
    engine
        .execute(Command::PlaceSheetPin { sheet_id, pin })
        .unwrap();
    engine
}

#[test]
fn the_same_commands_twice_give_the_same_document_and_ids() {
    let start = test_sheet();
    let first = run_steps(&start, &mut SequentialIds::default());
    let second = run_steps(&start, &mut SequentialIds::default());
    assert_eq!(first.serialize().unwrap(), second.serialize().unwrap());
    let doc = first.document();
    let wire_ids: Vec<Uuid> = doc.wires.iter().map(|w| w.uuid).collect();
    assert_eq!(wire_ids, [Uuid::from_u128(1), Uuid::from_u128(3)]);
    assert_eq!(doc.text_notes[0].uuid, Uuid::from_u128(2));
    assert_eq!(doc.child_sheets[0].uuid, Uuid::from_u128(4));
    assert_eq!(doc.child_sheets[0].pins[0].uuid, Uuid::from_u128(5));
}

#[test]
fn undo_then_redo_keeps_every_id() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let done = engine.serialize().unwrap();
    for _ in 0..4 {
        engine.undo().unwrap().expect("an undo step");
    }
    assert!(engine.document().wires.is_empty());
    for _ in 0..4 {
        engine.redo().unwrap().expect("a redo step");
    }
    assert_eq!(engine.serialize().unwrap(), done);
}

#[test]
fn break_wire_refuses_a_second_half_id_already_in_the_sheet() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let before = engine.serialize().unwrap();
    let wire = engine.document().wires[1].uuid;
    let taken = engine.document().text_notes[0].uuid;
    let refused = engine.execute(Command::BreakWire {
        wire,
        at: Point::new(25.0, 10.0),
        new_wire: taken,
    });
    assert!(matches!(refused, Err(EngineError::IdInUse(id)) if id == taken));
    assert_eq!(engine.serialize().unwrap(), before);
}

#[test]
fn a_paste_reusing_an_id_in_the_sheet_is_refused_whole() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let before = engine.serialize().unwrap();
    let depth = engine.undo_depth();
    // A fresh source repeats id 1, which the first paste already placed.
    let commands = payload()
        .place_commands(Point::new(10.0, 40.0), &mut SequentialIds::default())
        .unwrap();
    let refused = engine.execute_batch(commands);
    assert!(matches!(refused, Err(EngineError::IdInUse(id)) if id == Uuid::from_u128(1)));
    assert_eq!(engine.serialize().unwrap(), before);
    assert_eq!(engine.undo_depth(), depth);
}
