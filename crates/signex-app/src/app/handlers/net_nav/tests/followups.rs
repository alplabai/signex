//! Review follow-ups: failed builds retry, caches are released, the hover
//! timer stops while a build runs, history drops what it cannot open.

use std::time::{Duration, Instant};

use signex_types::schematic::{SelectedItem, SelectedKind};

use super::fixture::*;
use crate::app::NetNavMsg;
use crate::app::state::net_nav::hover::DWELL;

#[test]
fn a_command_retries_a_failed_build_once_the_root_is_fixed_on_disk() {
    let (mut p, root, label) = failing_root_project();
    p.select(label, SelectedKind::Label);
    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert!(
        p.app.document_state.net_nav.pending.is_none(),
        "the build failed"
    );

    // Fixed on disk: the project key did not move, but the user asked again.
    p.fix_root(&root);
    p.send(NetNavMsg::PartnerJumpRequested);

    assert!(
        p.app.document_state.net_nav.pending.is_some(),
        "an explicit command builds again instead of reusing the failure"
    );
}

#[test]
fn a_closed_projects_index_is_released_without_any_highlight() {
    let mut p = project();
    p.app.observe_net_hover(
        Some(SelectedItem::new(id(ROOT_VCC_WIRE), SelectedKind::Wire)),
        (5.0, 0.0),
    );
    p.app
        .document_state
        .net_nav
        .hover
        .candidate
        .as_mut()
        .expect("candidate")
        .since = Instant::now().checked_sub(DWELL * 2).expect("clock");
    p.send_settled(NetNavMsg::HoverDwellTicked);
    let a = p.app.document_state.projects[0].id;
    assert!(
        p.app.document_state.net_nav.index.latest(a).is_some(),
        "built for the hover"
    );
    assert!(p.app.document_state.net_nav.highlight.is_none());

    p.app.document_state.projects.retain(|x| x.id != a);
    p.send(NetNavMsg::HighlightClearRequested); // any update runs finish_update

    assert!(p.app.document_state.net_nav.index.latest(a).is_none());
}

#[test]
fn the_dwell_timer_stops_while_the_hover_waits_for_a_build() {
    let mut p = project();
    // An edit makes the cached index stale, so the dwell has to wait for it.
    p.focus(&p.mid.clone());
    p.select(23, SelectedKind::Wire);
    let _task = p.app.update(crate::app::Message::Edit(
        crate::app::EditMsg::DeleteSelected,
    ));
    p.focus(&p.root.clone());
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

    p.send(NetNavMsg::HoverDwellTicked);

    assert!(
        !p.app.document_state.net_nav.hover.needs_ticks(),
        "parked: no more ticks"
    );
    p.app.settle_net_index_blocking();
    assert!(
        p.app.document_state.net_nav.hover.card.is_some(),
        "and the card still comes"
    );
}

#[test]
fn history_drops_a_sheet_it_cannot_open_instead_of_skipping_it_every_pass() {
    let mut p = project();
    // `project()` opened third, then mid, then root. Close mid and delete it.
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
    p.focus(&p.root.clone());

    p.send(NetNavMsg::SheetBackRequested);
    assert_eq!(
        p.active(),
        Some(p.third.clone()),
        "mid skipped, third reached"
    );
    assert!(
        !p.app.document_state.net_nav.history.contains(&mid),
        "and forgotten"
    );

    p.send(NetNavMsg::SheetForwardRequested);
    assert_eq!(p.active(), Some(p.root.clone()));
    p.send(NetNavMsg::SheetBackRequested);
    assert_eq!(p.active(), Some(p.third.clone()));
}
