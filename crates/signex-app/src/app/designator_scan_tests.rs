//! Numbering a new part against the whole project (#1381).

use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;

/// `top` is open; `child` is placed twice by it and is NOT open. The child's
/// one symbol is R1 at placement A and R101 at placement B.
#[test]
fn a_closed_sheets_designators_are_not_reused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    assert!(!app.document_state.engines.contains_key(&child));

    app.scan_designators_blocking(&top);

    assert_eq!(app.next_designator_in_sheet(&top, "R"), "R102");
}

#[test]
fn a_typed_designator_steps_past_one_a_closed_sheet_uses() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    app.scan_designators_blocking(&top);

    assert_eq!(app.free_designator_from(&top, "R101"), "R102");
    assert_eq!(app.free_designator_from(&top, "R50"), "R50");
}

/// An open sheet counts at every placement it is reached through, not just
/// its base reference.
#[test]
fn an_open_sheet_counts_at_each_of_its_placements() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    app.open_or_focus_sheet_path(child.clone());
    assert!(app.document_state.engines.contains_key(&child));
    app.scan_designators_blocking(&top);

    assert_eq!(app.next_designator_in_sheet(&top, "R"), "R102");
}
