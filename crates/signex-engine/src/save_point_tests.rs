//! The save point (#1822): the history position the document held when it
//! last matched its file. Undo and redo move the document clean and dirty
//! again; an edit below the point makes it unreachable.

use signex_types::schematic::{Point, Symbol, Wire};

use crate::test_support::test_sheet;
use crate::{Command, Engine};

fn edit(engine: &mut Engine, i: usize) {
    let wire = Wire {
        color: None,
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

/// An engine whose content is known to equal its file.
fn saved_engine() -> Engine {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine.mark_saved();
    engine
}

#[test]
fn an_engine_not_known_to_match_a_file_is_not_at_a_save_point() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    assert!(!engine.is_at_save_point(), "unknown reads as dirty");
    edit(&mut engine, 1);
    engine.undo().unwrap();
    assert!(!engine.is_at_save_point());
}

#[test]
fn undoing_the_only_edit_returns_to_the_save_point_and_redo_leaves_it() {
    let mut engine = saved_engine();
    edit(&mut engine, 1);
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
    engine.redo().unwrap();
    assert!(!engine.is_at_save_point());
}

#[test]
fn a_save_then_an_edit_then_undo_is_at_the_new_save_point() {
    let mut engine = saved_engine();
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
    let mut engine = saved_engine();
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
    let mut engine = saved_engine();
    edit(&mut engine, 1);
    engine.mark_saved();
    engine.undo().unwrap();
    engine.redo().unwrap();
    assert!(engine.is_at_save_point());
    engine.forget_save_point();
    assert!(!engine.is_at_save_point());
}

/// How many recorded edits the engine keeps; read off the behaviour so the
/// guard does not depend on where the cap lives.
const KEPT: usize = 100;

fn undo_all(engine: &mut Engine) -> usize {
    let mut steps = 0;
    while engine.undo().unwrap().is_some() {
        steps += 1;
    }
    steps
}

#[test]
fn undo_position_counts_up_past_the_cap_and_back_down_to_the_evicted_floor() {
    let mut engine = saved_engine();
    for i in 0..KEPT + 7 {
        edit(&mut engine, i);
        assert_eq!(engine.undo_position(), i + 1, "monotonic across eviction");
    }
    assert_eq!(
        undo_all(&mut engine),
        KEPT,
        "only the cap's worth is undoable"
    );
    assert_eq!(engine.undo_position(), 7, "the evicted count is the floor");
    engine.redo().unwrap();
    assert_eq!(engine.undo_position(), 8);
}

#[test]
fn a_save_point_evicted_by_the_cap_is_never_clean_again() {
    let mut engine = saved_engine();
    for i in 0..3 {
        edit(&mut engine, i);
    }
    engine.mark_saved(); // position 3
    for i in 3..3 + KEPT + 5 {
        edit(&mut engine, i); // evicts 105 - 100 = 5 steps; 3 < 5
    }
    assert!(!engine.is_at_save_point());
    undo_all(&mut engine);
    assert!(
        !engine.is_at_save_point(),
        "undo cannot reach an evicted point"
    );
    engine.mark_saved();
    assert!(engine.is_at_save_point(), "a save re-anchors it");
}

#[test]
fn a_save_point_on_the_oldest_kept_state_stays_reachable() {
    let mut engine = saved_engine();
    for i in 0..5 {
        edit(&mut engine, i);
    }
    engine.mark_saved(); // position 5
    for i in 5..5 + KEPT {
        edit(&mut engine, i); // evicts exactly 5: the floor equals the point
    }
    assert!(!engine.is_at_save_point());
    assert_eq!(undo_all(&mut engine), KEPT);
    assert_eq!(engine.undo_position(), 5);
    assert!(
        engine.is_at_save_point(),
        "the bottom of the stack is the file"
    );
}

#[test]
fn a_save_point_that_survives_eviction_moves_with_the_positions() {
    let mut engine = saved_engine();
    for i in 0..KEPT {
        edit(&mut engine, i);
    }
    engine.mark_saved(); // position 100
    for i in 0..3 {
        edit(&mut engine, 200 + i); // evicts 3, the point is now 97 deep
    }
    for _ in 0..3 {
        engine.undo().unwrap();
    }
    assert!(engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(!engine.is_at_save_point());
}
