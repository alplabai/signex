//! The core half of a multi-sheet run's end (#2326 slice 2, PR 2.3, test
//! block C): its members read unsaved and repaint, its group is recorded,
//! and the ERC epoch moves once.

use signex_engine::{Command, DocumentPatch};

use super::SheetRun;
use crate::app::command::queries::tests::{WIRE_A, app_with, path, uid};
use crate::app::core::event::{ChangeCause, CoreEvent};
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::app::{Signex, UndoGroupKind, UndoGroupSheets};

fn colour_wire_a() -> Command {
    let mut colors = std::collections::HashMap::new();
    colors.insert(uid(WIRE_A), colours::color(8));
    Command::SetNetColors { colors }
}

/// `a` (active) and `b`, with `b` edited by a run that left `a` alone.
fn run_on_b(notes_edit: bool) -> (Signex, SheetRun) {
    let mut app = app_with(&[("b.snxsch", 101), ("a.snxsch", 100)]);
    let (a, b) = (path("a.snxsch"), path("b.snxsch"));
    assert_eq!(app.document_state.active_path.as_ref(), Some(&a));
    let group = app.mint_undo_group();
    let engine = app.document_state.engines.get_mut(&b).unwrap();
    engine.begin_transaction().unwrap();
    engine.execute(colour_wire_a()).unwrap();
    engine.commit_transaction(Some(group)).unwrap();
    let recorded = UndoGroupSheets {
        kind: UndoGroupKind::Replace,
        live: vec![b.clone()],
        ..Default::default()
    };
    let members = vec![(b, DocumentPatch::NET_COLORS)];
    let run = SheetRun {
        group,
        recorded,
        members,
        notes_edit,
    };
    (app, run)
}

#[test]
fn a_run_marks_its_member_unsaved_repaints_it_and_records_the_group() {
    let (mut app, run) = run_on_b(false);
    let (b, group) = (path("b.snxsch"), run.group);
    assert!(!app.document_state.is_document_dirty(&b));
    let events = app.document_state.settle_run(run, false);
    assert!(app.document_state.is_document_dirty(&b), "close must ask");
    assert!(
        events.iter().any(|event| matches!(
            event,
            CoreEvent::DocumentChanged(changed)
                if changed.path == b
                    && changed.patch == DocumentPatch::NET_COLORS
                    && changed.cause == ChangeCause::Edit
        )),
        "{events:?}"
    );
    assert_eq!(app.document_state.undo_groups[&group].live, vec![b]);
}

#[test]
fn a_run_owes_a_member_an_agent_edit_list_targets() {
    let (mut app, run) = run_on_b(false);
    let b = path("b.snxsch");
    app.document_state.agent_edits.target = Some(b.clone());
    let events = app.document_state.settle_run(run, false);
    assert!(events.is_empty(), "{events:?}");
    assert!(app.document_state.is_document_dirty(&b));
    assert_eq!(
        app.document_state.agent_edits.owed,
        DocumentPatch::NET_COLORS
    );
}

#[test]
fn a_run_that_touched_no_sheet_records_no_group() {
    let (mut app, mut run) = run_on_b(false);
    let group = run.group;
    run.members.clear();
    run.recorded.live.clear();
    app.document_state
        .undo_groups
        .insert(group, run.recorded.clone());
    let events = app.document_state.settle_run(run, false);
    assert!(events.is_empty());
    assert!(!app.document_state.undo_groups.contains_key(&group));
}

/// `notes_edit` moves the ERC epoch once, only when the active sheet's own
/// settle does not.
#[test]
fn a_run_notes_the_sheet_edit_only_when_the_active_sheet_took_none() {
    for (notes_edit, active_changed, bump) in [(true, false, 1), (true, true, 0), (false, false, 0)]
    {
        let (mut app, mut run) = run_on_b(notes_edit);
        run.members.clear();
        let epoch = app.document_state.sheet_edit_epoch;
        let _events = app.document_state.settle_run(run, active_changed);
        assert_eq!(
            app.document_state.sheet_edit_epoch,
            epoch + bump,
            "notes_edit {notes_edit}, active_changed {active_changed}"
        );
    }
}
