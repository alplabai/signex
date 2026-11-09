//! #661 slice 6: a net colour on a member sheet is a later edit of that
//! sheet, and the sheets a bulk pass records reach their save point by undo.

use super::annotate_tests::{project, root_refs, unit};
use crate::app::handlers::editing_commands::net_color_driver::{color, paint_on};
use crate::app::path_ops::settle_task;

fn open_child(app: &mut crate::app::Signex, dir: &std::path::Path) -> std::path::PathBuf {
    let child = dir.join("z_child.snxsch");
    let engine = signex_engine::Engine::open(&child).expect("child opens");
    app.document_state.engines.insert(child.clone(), engine);
    child
}

#[test]
fn a_colour_step_on_a_member_sheet_blocks_a_project_wide_undo() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    app.ui_state.annotate_undo = crate::annotate_undo::AnnotateUndo::Project;
    let child = open_child(&mut app, &dir);
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    settle_task(&mut app, task);
    let numbered = root_refs(&app, &dir);
    let wire = app.document_state.engines[&child].document().wires[0].uuid;

    paint_on(&mut app, &child, &[wire], color(1));
    let moved = app.apply_engine_undo(true);

    let after = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(!moved, "the colour is a newer edit of a member sheet");
    assert_eq!(after, numbered, "the root sheet is still numbered");
}

#[test]
fn undoing_a_reset_on_a_parked_sheet_back_to_its_save_point_reads_clean() {
    let (mut app, dir) = project(vec![unit("R1", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
    let child = open_child(&mut app, &dir);
    let task = app.handle_reset_duplicate_designators();
    settle_task(&mut app, task);
    assert!(app.document_state.is_document_dirty(&child), "reset it");

    let engine = app.document_state.engines.get_mut(&child).expect("child");
    assert!(engine.undo().expect("undo").is_some());
    app.document_state.sync_history_dirty(&child);

    let dirty = app.document_state.is_document_dirty(&child);
    std::fs::remove_dir_all(&dir).ok();
    assert!(!dirty, "the history is back at the save point");
}
