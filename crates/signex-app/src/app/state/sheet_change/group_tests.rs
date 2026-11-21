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

/// `a` (active) and `b` under one colour group, then a later colour on `b`.
fn blocked_group() -> (Signex, UndoGroupId, PathBuf, PathBuf) {
    let mut app = app_with(&[("b.snxsch", 101), ("a.snxsch", 100)]);
    let (a, b) = (path("a.snxsch"), path("b.snxsch"));
    let group = app.mint_undo_group();
    for sheet in [&a, &b] {
        let engine = app.document_state.engines.get_mut(sheet).unwrap();
        engine.begin_transaction().unwrap();
        engine.execute(colour_wire_a(8)).unwrap();
        engine.commit_transaction(Some(group)).unwrap();
    }
    let record = UndoGroupSheets {
        kind: UndoGroupKind::Replace,
        live: vec![a.clone(), b.clone()],
        ..Default::default()
    };
    app.document_state.undo_groups.insert(group, record);
    let later = app.document_state.engines.get_mut(&b).unwrap();
    later.execute(colour_wire_a(9)).unwrap();
    (app, group, a, b)
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

/// The multi-sheet finishers mark a non-active member with
/// `mark_history_edit_dirty`; a member whose engine is gone still reads
/// unsaved, so close and quit still ask.
#[test]
fn a_run_member_with_no_engine_is_still_marked_dirty() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let gone = path("gone.snxsch");
    assert!(!app.document_state.engines.contains_key(&gone));
    app.document_state.mark_history_edit_dirty(&gone);
    assert!(app.document_state.is_document_dirty(&gone));
}
