//! The core half of a grouped step on its own (#2326 slice 2, PR 2.3, test
//! block C): what keeps a parked member's engine, a refused step, and the
//! dirty mark of a multi-sheet run's member with no engine.

use std::collections::HashSet;
use std::path::PathBuf;

use signex_engine::{Command, UndoGroupId};

use crate::app::Signex;
use crate::app::command::queries::tests::{WIRE_A, app_with, path, uid};
use crate::app::core::event::{CoreEvent, NoticeLevel};
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::app::sheet_numbering::apply_tests::{Rig, apply, closed_parent_sheet, rig};
use crate::app::{UndoGroupKind, UndoGroupSheets};

fn colour_wire_a(r: u8) -> Command {
    let mut colors = std::collections::HashMap::new();
    colors.insert(uid(WIRE_A), colours::color(r));
    Command::SetNetColors { colors }
}

/// Number Schematic Sheets over an open, active parent and a closed one the
/// run parks: the parked one is the group's non-active member.
fn renumbered() -> (Rig, UndoGroupId) {
    let mut rig = rig(Some(closed_parent_sheet()));
    let open = rig.open_parent.clone();
    let app = &mut rig.app;
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "a.snxsch".into(),
        path: open.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(open.clone());
    apply(&mut rig);
    let group = rig.app.document_state.engines[&open].next_undo_group();
    (rig, group.expect("the run stamped a group"))
}

#[test]
fn an_undo_keeps_a_parked_member_an_undocked_window_shows() {
    let (mut rig, group) = renumbered();
    let parked = rig.closed_parent.clone();
    let undocked: HashSet<PathBuf> = [parked.clone()].into_iter().collect();
    let step = rig
        .app
        .document_state
        .step_undo_group(group, true, &undocked);
    assert!(step.ran && step.stepped, "the group moved: {step:?}");
    assert!(
        rig.app.document_state.engines.contains_key(&parked),
        "an undocked window holds it: its engine stays"
    );
    assert!(
        step.members.iter().any(|event| matches!(
            event, CoreEvent::DocumentChanged(changed) if changed.path == parked
        )),
        "the window showing it repaints"
    );
    assert!(
        rig.app.document_state.undo_groups[&group]
            .dropped
            .is_empty()
    );
}

#[test]
fn an_undo_keeps_a_parked_member_the_user_opened_in_a_tab() {
    let (mut rig, group) = renumbered();
    let parked = rig.closed_parent.clone();
    rig.app.document_state.tabs.push(crate::app::TabInfo {
        title: "parked.snxsch".into(),
        path: parked.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    let step = rig
        .app
        .document_state
        .step_undo_group(group, true, &HashSet::new());
    assert!(step.ran && step.stepped, "the group moved: {step:?}");
    assert!(
        rig.app.document_state.engines.contains_key(&parked),
        "a tab holds it: its engine stays"
    );
    assert!(
        step.members.iter().any(|event| matches!(
            event, CoreEvent::DocumentChanged(changed) if changed.path == parked
        )),
        "the tab showing it repaints"
    );
    assert!(
        rig.app.document_state.undo_groups[&group]
            .dropped
            .is_empty()
    );
}

#[test]
fn an_undo_drops_a_parked_member_nothing_holds_open() {
    let (mut rig, group) = renumbered();
    let parked = rig.closed_parent.clone();
    let step = rig
        .app
        .document_state
        .step_undo_group(group, true, &HashSet::new());
    assert!(step.ran && step.stepped, "the group moved: {step:?}");
    assert!(!rig.app.document_state.engines.contains_key(&parked));
    assert!(!rig.app.document_state.is_document_dirty(&parked));
    assert_eq!(
        rig.app.document_state.undo_groups[&group].dropped,
        vec![parked]
    );
}

/// One colour on each of `sheets` under one group.
fn colour_group(app: &mut Signex, sheets: &[&PathBuf]) -> UndoGroupId {
    let group = app.mint_undo_group();
    for sheet in sheets {
        let engine = app.document_state.engines.get_mut(*sheet).unwrap();
        engine.begin_transaction().unwrap();
        engine.execute(colour_wire_a(8)).unwrap();
        engine.commit_transaction(Some(group)).unwrap();
    }
    let record = UndoGroupSheets {
        kind: UndoGroupKind::Replace,
        live: sheets.iter().map(|sheet| (*sheet).clone()).collect(),
        ..Default::default()
    };
    app.document_state.undo_groups.insert(group, record);
    group
}

/// The active sheet's change is owed to a running agent edit list: the
/// history moved all the same, so the library drift check is still asked
/// for, as the grouped step always did.
#[test]
fn a_group_step_whose_active_change_is_owed_still_asks_for_the_drift_check() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let a = path("a.snxsch");
    let group = colour_group(&mut app, &[&a]);
    app.document_state.agent_edits.target = Some(path("other.snxsch"));
    app.library.drift.check_pending = false;
    let step = app
        .document_state
        .step_undo_group(group, true, &HashSet::new());
    assert!(step.stepped, "owed counts as a change");
    app.project_group_step(step, true);
    assert!(app.library.drift.check_pending);
}

/// `a` (active) and `b` under one colour group, then a later colour on `b`.
fn blocked_group() -> (Signex, UndoGroupId, PathBuf, PathBuf) {
    let mut app = app_with(&[("b.snxsch", 101), ("a.snxsch", 100)]);
    let (a, b) = (path("a.snxsch"), path("b.snxsch"));
    let group = colour_group(&mut app, &[&a, &b]);
    let later = app.document_state.engines.get_mut(&b).unwrap();
    later.execute(colour_wire_a(9)).unwrap();
    (app, group, a, b)
}

/// A non-active member is the target of a running agent edit list: its
/// refresh is owed to the list's end, not shown now. The active sheet is not
/// in the group, so all the owed patch is the member's.
#[test]
fn a_group_step_owes_a_member_an_agent_edit_list_targets() {
    let mut app = app_with(&[("b.snxsch", 101), ("a.snxsch", 100)]);
    let b = path("b.snxsch");
    let group = colour_group(&mut app, &[&b]);
    assert_eq!(
        app.document_state.agent_edits.owed,
        signex_engine::DocumentPatch::NONE
    );
    app.document_state.agent_edits.target = Some(b.clone());
    let step = app
        .document_state
        .step_undo_group(group, true, &HashSet::new());
    assert!(step.ran, "the group moved: {step:?}");
    assert_eq!(app.document_state.engines[&b].next_undo_group(), None);
    assert!(
        !step.members.iter().any(|event| matches!(
            event, CoreEvent::DocumentChanged(changed) if changed.path == b
        )),
        "owed, not shown: {:?}",
        step.members
    );
    assert!(
        (app.document_state.agent_edits.owed).contains(signex_engine::DocumentPatch::NET_COLORS)
    );
}

#[test]
fn a_blocked_group_step_returns_only_a_notice_and_moves_nothing() {
    let (mut app, group, a, b) = blocked_group();
    let epoch = app.document_state.sheet_edit_epoch;
    let step = app
        .document_state
        .step_undo_group(group, true, &HashSet::new());
    assert!(!step.ran && !step.stepped);
    assert!(step.active.is_empty(), "{:?}", step.active);
    let levels: Vec<_> = (step.members.iter())
        .map(|event| match event {
            CoreEvent::Notice(notice) => notice.level,
            CoreEvent::DocumentChanged(changed) => panic!("moved {changed:?}"),
        })
        .collect();
    assert_eq!(levels, vec![NoticeLevel::Warning]);
    assert_eq!(
        app.document_state.engines[&a].next_undo_group(),
        Some(group)
    );
    assert_eq!(app.document_state.engines[&b].next_undo_group(), None);
    assert_eq!(app.document_state.sheet_edit_epoch, epoch, "no edit noted");
}

/// A multi-sheet run's member whose engine is gone still reads unsaved once
/// the run settles, so close and quit still ask.
#[test]
fn a_run_member_with_no_engine_is_still_marked_dirty() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let gone = path("gone.snxsch");
    assert!(!app.document_state.engines.contains_key(&gone));
    let run = super::SheetRun {
        group: app.mint_undo_group(),
        recorded: UndoGroupSheets {
            kind: UndoGroupKind::Replace,
            live: vec![gone.clone()],
            ..Default::default()
        },
        members: vec![(gone.clone(), signex_engine::DocumentPatch::NET_COLORS)],
        notes_edit: false,
    };
    let _events = app.document_state.settle_run(run, false);
    assert!(app.document_state.is_document_dirty(&gone));
}
