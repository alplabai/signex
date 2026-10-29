//! Review round: a parked command survives a hover tick, a highlight never
//! moves to an unrelated net, placements are per instance, and index warnings
//! reach Messages once.

use std::time::{Duration, Instant};

use signex_types::schematic::{InstancePath, SelectedItem, SelectedKind};

use super::fixture::*;
use crate::app::state::net_nav::hover::DWELL;
use crate::app::{EditMsg, Message, NetNavMsg};

fn delete_selected(p: &mut Project) {
    let _task = p.app.update(Message::Edit(EditMsg::DeleteSelected));
}

#[test]
fn a_hover_tick_during_the_build_does_not_drop_a_parked_jump() {
    let mut p = project();
    // A real edit makes the cached index stale.
    p.focus(&p.mid.clone());
    p.select(23, SelectedKind::Wire);
    delete_selected(&mut p);
    p.focus(&p.root.clone());
    p.select(ROOT_VCC, SelectedKind::Label);
    p.app.observe_net_hover(
        Some(SelectedItem::new(id(ROOT_VCC), SelectedKind::Label)),
        (0.0, 0.0),
    );
    p.app
        .document_state
        .net_nav
        .hover
        .candidate
        .as_mut()
        .expect("candidate")
        .since = Instant::now()
        .checked_sub(DWELL + Duration::from_millis(50))
        .expect("clock");

    p.send(NetNavMsg::PartnerJumpRequested);
    p.send(NetNavMsg::HoverDwellTicked);
    p.app.settle_net_index_blocking();

    assert_eq!(p.active(), Some(p.mid.clone()), "the jump still ran");
}

#[test]
fn deleting_the_anchor_never_moves_the_highlight_to_an_unrelated_net() {
    let mut p = project();
    p.select(R5_WIRE, SelectedKind::Wire);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    let name = p
        .app
        .document_state
        .net_nav
        .highlight
        .as_ref()
        .map(|h| h.name.clone())
        .expect("highlight");
    assert!(name.starts_with("N$"), "{name} is auto-named");

    // R5's net vanishes; R4's net is renumbered into its auto-name.
    p.select(R5_WIRE, SelectedKind::Wire);
    delete_selected(&mut p);
    p.app.settle_net_index_blocking();

    assert!(p.app.document_state.net_nav.highlight.is_none());
    assert!(p.bright().is_none());
    assert!(logged(&["no longer exists"]));
}

#[test]
fn a_successful_resolve_moves_the_anchor_to_a_member_that_still_exists() {
    let mut p = project();
    p.select(ROOT_VCC_WIRE, SelectedKind::Wire);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    p.select(ROOT_VCC_WIRE, SelectedKind::Wire);
    delete_selected(&mut p);
    p.app.settle_net_index_blocking();

    let h = p
        .app
        .document_state
        .net_nav
        .highlight
        .as_ref()
        .expect("VCC survives");
    assert_eq!(h.name, "VCC");
    assert_ne!(
        h.anchor.1,
        id(ROOT_VCC_WIRE),
        "the deleted wire is no anchor"
    );
}

#[test]
fn hover_and_partners_follow_the_placement_the_tab_was_entered_through() {
    let (mut p, child) = placed_twice();
    let via_b = InstancePath::from_chain(vec![id(TWICE_B)]);
    p.app.open_sheet_with_placement(child.clone(), Some(via_b));
    assert_eq!(p.active(), Some(child));

    p.app.observe_net_hover(
        Some(SelectedItem::new(id(TWICE_LABEL), SelectedKind::Label)),
        (0.0, 70.0),
    );
    p.app
        .document_state
        .net_nav
        .hover
        .candidate
        .as_mut()
        .expect("candidate")
        .since = Instant::now()
        .checked_sub(DWELL + Duration::from_millis(50))
        .expect("clock");
    p.send_settled(NetNavMsg::HoverDwellTicked);
    let card = p
        .app
        .document_state
        .net_nav
        .hover
        .card
        .clone()
        .expect("card");
    assert!(
        card.name.starts_with("B/"),
        "instance B's net, got {}",
        card.name
    );

    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert_eq!(p.active(), Some(p.root.clone()));
    assert_eq!(
        p.selected().first().map(|i| i.uuid),
        Some(id(TWICE_ENTRY_B))
    );
}

#[test]
fn an_unreadable_page_is_reported_once_however_often_the_index_rebuilds() {
    let mut p = project_with_broken_page();
    p.select(ROOT_VCC, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    // An edit forces a second build.
    p.select(R5_WIRE, SelectedKind::Wire);
    delete_selected(&mut p);
    p.app.settle_net_index_blocking();

    assert_eq!(crate::diagnostics::logged_count(BROKEN), 1);
}
