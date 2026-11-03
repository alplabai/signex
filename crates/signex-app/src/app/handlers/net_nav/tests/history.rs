//! Back, forward and up through the sheets (#1272).

use signex_types::schematic::SelectedKind;

use super::fixture::*;
use crate::app::NetNavMsg;

#[test]
fn back_and_forward_walk_the_sheets_in_the_order_visited() {
    let mut p = project();
    // `project()` opened third, then mid, then root.
    p.send(NetNavMsg::SheetBackRequested);
    assert_eq!(p.active(), Some(p.mid.clone()));
    p.send(NetNavMsg::SheetBackRequested);
    assert_eq!(p.active(), Some(p.third.clone()));
    p.send(NetNavMsg::SheetForwardRequested);
    assert_eq!(p.active(), Some(p.mid.clone()));
    p.send(NetNavMsg::SheetForwardRequested);
    assert_eq!(p.active(), Some(p.root.clone()));
}

#[test]
fn back_at_the_oldest_sheet_stays_and_says_there_is_nothing_earlier() {
    let mut p = project();
    for _ in 0..2 {
        p.send(NetNavMsg::SheetBackRequested);
    }
    assert_eq!(p.active(), Some(p.third.clone()));

    p.send(NetNavMsg::SheetBackRequested);

    assert_eq!(p.active(), Some(p.third.clone()));
    assert!(logged(&["no earlier sheet"]));
}

#[test]
fn a_jump_after_going_back_drops_the_forward_list() {
    let mut p = project();
    p.send(NetNavMsg::SheetBackRequested);
    assert_eq!(p.active(), Some(p.mid.clone()));
    p.select(MID_VCC, SelectedKind::Label);
    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert_eq!(p.active(), Some(p.third.clone()));

    p.send(NetNavMsg::SheetForwardRequested);

    assert_eq!(p.active(), Some(p.third.clone()), "nothing later to go to");
    p.send(NetNavMsg::SheetBackRequested);
    assert_eq!(p.active(), Some(p.mid.clone()));
}

#[test]
fn up_lands_on_the_sheet_symbol_that_instantiates_the_sheet() {
    let mut p = project();
    p.focus(&p.mid.clone());

    p.app.ascend_settled();

    assert_eq!(p.active(), Some(p.root.clone()));
    let picked: Vec<_> = p.selected().iter().map(|i| (i.uuid, i.kind)).collect();
    assert_eq!(picked, [(id(MID_SYMBOL), SelectedKind::ChildSheet)]);
}

/// Delete `mid`, close its tab and start a Back step to it (its read is in
/// flight, not delivered); returns the generation that read carries.
fn start_back_to_deleted_mid(p: &mut Project) -> u64 {
    let mid = p.mid.clone();
    let tab = p
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == mid)
        .expect("mid tab");
    p.app.document_state.tabs.remove(tab);
    p.app.document_state.engines.remove(&mid);
    std::fs::remove_file(&mid).expect("delete mid");
    p.send(NetNavMsg::SheetBackRequested);
    p.app
        .document_state
        .pending_opens
        .generation_of(&mid)
        .expect("the step's read is in flight")
}

#[test]
fn a_failed_history_read_leaves_an_entry_that_is_no_longer_the_one_asked_for() {
    let mut p = project();
    let mid = p.mid.clone();
    let generation = start_back_to_deleted_mid(&mut p);
    // The user focuses another sheet before the read fails: a visit, so the
    // top of the back stack is now the sheet left, not `mid`.
    let (third, root) = (p.third.clone(), p.root.clone());
    p.focus(&third);
    assert_eq!(
        p.app.document_state.net_nav.history.peek(false),
        Some(root.as_path())
    );

    let _ = p.app.sheet_read_failed(&mid, generation, true);

    let history = &p.app.document_state.net_nav.history;
    assert_eq!(
        history.peek(false),
        Some(root.as_path()),
        "root's entry survives"
    );
    assert_eq!(p.active(), Some(third), "and no second step was taken");
}

#[test]
fn a_second_open_of_a_sheet_being_read_says_it_replaced_the_first() {
    let mut p = project();
    let mid = p.mid.clone();
    start_back_to_deleted_mid(&mut p);
    let _ = p.app.open_sheet_with_placement(mid, None);
    assert!(logged(&["asked to open again", "history step"]));
}

#[test]
fn a_history_read_landing_after_the_user_moved_on_keeps_the_stacks() {
    use crate::app::{Message, UiMsg};
    let mut p = project();
    let (mid, third, root) = (p.mid.clone(), p.third.clone(), p.root.clone());
    let tab = p
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == mid)
        .expect("mid tab");
    p.app.document_state.tabs.remove(tab);
    p.app.document_state.engines.remove(&mid);
    let step = p
        .app
        .update(Message::Ui(UiMsg::NetNav(NetNavMsg::SheetBackRequested)));
    p.focus(&third);
    assert_eq!(
        p.app.document_state.net_nav.history.peek(false),
        Some(root.as_path())
    );

    crate::app::path_ops::settle_task(&mut p.app, step);

    assert_eq!(
        p.app.document_state.net_nav.history.peek(false),
        Some(root.as_path()),
        "root's back entry survives the late step"
    );
    assert_eq!(p.active(), Some(third), "and the view stays put");
}

#[test]
fn a_second_back_before_the_first_is_read_does_not_replace_it() {
    let mut p = project();
    let mid = p.mid.clone();
    let tab = p
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == mid)
        .expect("mid tab");
    p.app.document_state.tabs.remove(tab);
    p.app.document_state.engines.remove(&mid);
    p.send(NetNavMsg::SheetBackRequested);
    let first = p.app.document_state.pending_opens.generation_of(&mid);
    assert!(first.is_some());

    p.send(NetNavMsg::SheetBackRequested);

    assert_eq!(
        p.app.document_state.pending_opens.generation_of(&mid),
        first
    );
    assert_eq!(p.app.ui_state.pending_sheet_opens.len(), 1);
}
