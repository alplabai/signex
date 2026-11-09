//! The undo depth (#661): one setting for how many steps the sheet keeps.

use signex_types::schematic::{Point, Wire};

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
    engine.execute(Command::PlaceWireSegment { wire }).unwrap();
}

fn engine_with_edits(depth: usize, edits: usize) -> Engine {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine.mark_saved();
    engine.set_undo_depth(depth);
    for i in 0..edits {
        edit(&mut engine, i);
    }
    engine
}

fn undo_all(engine: &mut Engine) -> usize {
    let mut steps = 0;
    while engine.undo().unwrap().is_some() {
        steps += 1;
    }
    steps
}

#[test]
fn a_new_engine_keeps_the_default_hundred() {
    assert_eq!(Engine::new(test_sheet()).unwrap().undo_depth(), 100);
}

#[test]
fn depth_one_keeps_one_step_and_a_thousand_keeps_a_thousand() {
    for depth in [1, 100, 1000] {
        let mut engine = engine_with_edits(depth, depth + 3);
        assert_eq!(undo_all(&mut engine), depth, "depth {depth}");
        assert_eq!(engine.undo_position(), 3, "the evicted steps still count");
        assert_eq!(engine.document().wires.len(), 3 + test_sheet().wires.len());
    }
}

#[test]
fn lowering_the_depth_drops_the_oldest_steps_at_once() {
    let mut engine = engine_with_edits(100, 10);
    let base = test_sheet().wires.len();
    engine.set_undo_depth(4);
    assert_eq!(engine.undo_position(), 10, "the position does not move");
    assert_eq!(undo_all(&mut engine), 4);
    assert_eq!(
        engine.document().wires.len(),
        base + 6,
        "the newest four undone"
    );
}

#[test]
fn lowering_the_depth_trims_the_far_end_of_redo() {
    let mut engine = engine_with_edits(100, 6);
    undo_all(&mut engine);
    engine.set_undo_depth(2);
    let mut redone = 0;
    while engine.redo().unwrap().is_some() {
        redone += 1;
    }
    assert_eq!(redone, 2);
}

#[test]
fn a_save_point_trimmed_by_a_lower_depth_never_reads_clean_again() {
    let mut engine = engine_with_edits(100, 2);
    engine.mark_saved(); // position 2
    for i in 2..10 {
        edit(&mut engine, i);
    }
    engine.set_undo_depth(4); // keeps positions 6..=10
    assert!(!engine.is_at_save_point());
    undo_all(&mut engine);
    assert!(
        !engine.is_at_save_point(),
        "the saved state is trimmed away"
    );
    engine.mark_saved();
    assert!(engine.is_at_save_point(), "a save re-anchors it");
}

#[test]
fn a_save_point_inside_the_kept_steps_survives_a_lower_depth() {
    let mut engine = engine_with_edits(100, 8);
    engine.mark_saved(); // position 8
    edit(&mut engine, 8);
    edit(&mut engine, 9);
    engine.set_undo_depth(4); // keeps positions 6..=10
    engine.undo().unwrap();
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
}
