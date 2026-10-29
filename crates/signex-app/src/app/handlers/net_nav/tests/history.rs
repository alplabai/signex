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
    assert!(logged(&["No earlier sheet"]));
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

    p.app.ascend_to_parent_sheet();

    assert_eq!(p.active(), Some(p.root.clone()));
    let picked: Vec<_> = p.selected().iter().map(|i| (i.uuid, i.kind)).collect();
    assert_eq!(picked, [(id(MID_SYMBOL), SelectedKind::ChildSheet)]);
}
