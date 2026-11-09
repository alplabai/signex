//! Net-class pick markers (#661): an ordering step of the sheet's timeline
//! that changes nothing on the sheet, so it never moves the save point.

use signex_types::schematic::{Point, Wire};

use crate::test_support::test_sheet;
use crate::{ClassPickMarker, Command, Engine};

const MARK: ClassPickMarker = ClassPickMarker {
    project: 1,
    step: 7,
};

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

fn saved() -> Engine {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine.mark_saved();
    engine
}

#[test]
fn a_marker_alone_does_not_dirty_the_sheet() {
    let mut engine = saved();
    engine.record_class_pick(MARK);
    assert!(engine.is_at_save_point());
    assert!(engine.pass_class_pick(true));
    assert!(engine.is_at_save_point());
    assert!(engine.pass_class_pick(false));
    assert!(engine.is_at_save_point());
}

#[test]
fn an_edit_after_a_marker_dirties_and_undoing_it_reads_clean_again() {
    let mut engine = saved();
    engine.record_class_pick(MARK);
    edit(&mut engine, 1);
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(engine.is_at_save_point(), "only the marker is left above");
    engine.redo().unwrap();
    assert!(!engine.is_at_save_point());
}

#[test]
fn an_edit_before_a_marker_is_still_dirty_with_the_marker_on_top() {
    let mut engine = saved();
    edit(&mut engine, 1);
    engine.record_class_pick(MARK);
    assert!(!engine.is_at_save_point());
    assert!(engine.pass_class_pick(true));
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
}

#[test]
fn a_save_taken_over_a_marker_keeps_it_clean_both_ways() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    edit(&mut engine, 1);
    engine.record_class_pick(MARK);
    engine.mark_saved();
    assert!(engine.is_at_save_point());
    assert!(engine.pass_class_pick(true));
    assert!(engine.is_at_save_point(), "the marker is no sheet change");
    engine.undo().unwrap();
    assert!(!engine.is_at_save_point());
}

#[test]
fn undo_stops_at_a_marker_until_it_is_passed() {
    let mut engine = saved();
    edit(&mut engine, 1);
    engine.record_class_pick(MARK);
    assert_eq!(engine.next_class_pick(true), Some(MARK));
    assert!(engine.undo().unwrap().is_none(), "the app steps markers");
    assert!(engine.pass_class_pick(true));
    assert_eq!(engine.next_class_pick(true), None);
    assert_eq!(engine.next_class_pick(false), Some(MARK));
    assert!(engine.undo().unwrap().is_some());
}

#[test]
fn a_marker_shares_the_undo_depth_and_falls_off_oldest_first() {
    let mut engine = saved();
    engine.set_undo_depth(2);
    engine.record_class_pick(MARK);
    edit(&mut engine, 1);
    edit(&mut engine, 2);
    assert!(engine.undo().unwrap().is_some());
    assert!(engine.undo().unwrap().is_some());
    assert_eq!(engine.next_class_pick(true), None, "the marker was evicted");
    assert!(engine.undo().unwrap().is_none());
}

#[test]
fn an_edit_ends_the_marker_redo_branch() {
    let mut engine = saved();
    engine.record_class_pick(MARK);
    assert!(engine.pass_class_pick(true));
    edit(&mut engine, 1);
    assert_eq!(engine.next_class_pick(false), None);
    assert!(!engine.can_redo());
}

#[test]
fn a_marker_has_no_undo_group_and_a_document_step_is_told_apart() {
    let mut engine = saved();
    assert!(!engine.has_document_step(true));
    engine.record_class_pick(MARK);
    assert!(engine.can_undo() && !engine.has_document_step(true));
    edit(&mut engine, 1);
    assert!(engine.has_document_step(true));
    let kinds: Vec<_> = engine.steps_from_top(true).collect();
    assert_eq!(kinds, [None, Some(MARK)]);
}

#[test]
fn a_marker_keeps_the_redo_branch_and_the_save_point_on_it() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    edit(&mut engine, 1);
    engine.mark_saved(); // saved with the edit applied
    engine.undo().unwrap();
    assert!(!engine.is_at_save_point());

    engine.record_class_pick(MARK);

    assert!(engine.can_redo(), "a pick does not change the sheet");
    assert!(!engine.is_at_save_point(), "the saved edit is still ahead");
    assert!(engine.redo().unwrap().is_some());
    assert!(engine.is_at_save_point(), "redo reaches the saved state");
    engine.undo().unwrap();
    assert_eq!(
        engine.next_class_pick(true),
        Some(MARK),
        "the marker is below"
    );
}

#[test]
fn a_marker_over_an_unchanged_redo_branch_of_markers_stays_clean() {
    let mut engine = saved();
    engine.record_class_pick(MARK);
    engine.mark_saved();
    assert!(engine.pass_class_pick(true));
    engine.record_class_pick(ClassPickMarker { step: 8, ..MARK });
    assert!(engine.is_at_save_point());
}

#[test]
fn an_edit_cutting_a_redo_branch_that_holds_the_save_point_forgets_it() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    edit(&mut engine, 1);
    engine.mark_saved();
    engine.undo().unwrap();
    edit(&mut engine, 2); // cuts the saved state off the redo branch
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(
        !engine.is_at_save_point(),
        "unreachable until the next save"
    );
}

#[test]
fn a_dead_marker_passes_over_as_one_that_can_never_match() {
    let mut engine = saved();
    engine.record_class_pick(MARK);
    assert!(engine.pass_dead_class_pick(true));
    assert_eq!(engine.next_class_pick(false), Some(MARK.dead()));
    assert_eq!(MARK.dead().project, MARK.project);
    assert!(engine.pass_dead_class_pick(false));
    assert_eq!(
        engine.next_class_pick(true),
        Some(MARK.dead()),
        "stays dead both ways"
    );
    assert!(!engine.pass_dead_class_pick(false), "nothing left to pass");
    assert!(!engine.pass_class_pick(false));
}

/// Known edge: a marker passed back onto a full undo stack pushes the oldest
/// step off, and a save point under it is forgotten, though only markers lay
/// between. It fails toward dirty only.
#[test]
fn passing_a_marker_onto_a_full_stack_can_drop_the_save_point() {
    let mut engine = saved();
    engine.set_undo_depth(2);
    engine.record_class_pick(MARK);
    assert!(engine.pass_class_pick(true));
    engine.record_class_pick(ClassPickMarker { step: 8, ..MARK });
    engine.record_class_pick(ClassPickMarker { step: 9, ..MARK });
    assert!(
        engine.is_at_save_point(),
        "markers only: still the saved sheet"
    );

    assert!(engine.pass_dead_class_pick(false));

    assert!(
        !engine.is_at_save_point(),
        "the saved depth fell off the stack"
    );
}
