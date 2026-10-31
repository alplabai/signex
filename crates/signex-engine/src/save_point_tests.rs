//! The save point (#1822): the history position the document held when it
//! last matched its file. Undo and redo move the document clean and dirty
//! again; an edit below the point makes it unreachable.

use signex_types::schematic::{Point, Symbol, Wire};

use crate::test_support::test_sheet;
use crate::{Command, Engine};

fn edit(engine: &mut Engine, i: usize) {
    let wire = Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(i as f64, 0.0),
        end: Point::new(i as f64, 5.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    assert!(
        engine
            .execute(Command::PlaceWireSegment { wire })
            .unwrap()
            .changed
    );
}

#[test]
fn a_fresh_engine_is_at_its_save_point() {
    let engine = Engine::new(test_sheet()).unwrap();
    assert!(engine.is_at_save_point());
}

#[test]
fn undoing_the_only_edit_returns_to_the_save_point_and_redo_leaves_it() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    edit(&mut engine, 1);
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
    engine.redo().unwrap();
    assert!(!engine.is_at_save_point());
}

#[test]
fn a_save_then_an_edit_then_undo_is_at_the_new_save_point() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    edit(&mut engine, 1);
    engine.mark_saved();
    assert!(engine.is_at_save_point());
    edit(&mut engine, 2);
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(!engine.is_at_save_point(), "the file holds the first edit");
}

#[test]
fn an_edit_below_the_save_point_makes_it_unreachable() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    edit(&mut engine, 1);
    engine.mark_saved();
    engine.undo().unwrap();
    // The new edit lands at the saved position but is a different step.
    edit(&mut engine, 2);
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(!engine.is_at_save_point(), "undo cannot reach the file");
    engine.redo().unwrap();
    assert!(!engine.is_at_save_point());
    engine.mark_saved();
    assert!(engine.is_at_save_point());
}

#[test]
fn a_cell_run_that_amends_the_saved_entry_is_dirty() {
    let id = uuid::Uuid::now_v7();
    let mut sheet = test_sheet();
    sheet.symbols.push(Symbol {
        uuid: id,
        ..Default::default()
    });
    let mut engine = Engine::new(sheet).unwrap();
    let set = |engine: &mut Engine, value: &str| {
        engine
            .execute(Command::SetSymbolField {
                symbol_id: id,
                key: "MPN".into(),
                value: value.into(),
            })
            .unwrap();
    };
    set(&mut engine, "A");
    engine.mark_saved();
    // Same cell, same run: the entry the file was saved at is rewritten.
    set(&mut engine, "AB");
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(
        !engine.is_at_save_point(),
        "the file holds \"A\", not the base"
    );
}

#[test]
fn forgetting_the_save_point_leaves_nothing_clean() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    edit(&mut engine, 1);
    engine.mark_saved();
    engine.undo().unwrap();
    engine.redo().unwrap();
    assert!(engine.is_at_save_point());
    engine.forget_save_point();
    assert!(!engine.is_at_save_point());
}
