//! The hover insight (#1270).

use std::time::{Duration, Instant};

use signex_types::net_class::NetClass;
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::fixture::*;
use crate::app::NetNavMsg;
use crate::app::state::net_nav::hover::DWELL;

/// Rest the cursor on `uuid`, let the dwell pass and answer the timer.
fn dwell_on(p: &mut Project, uuid: u128, kind: SelectedKind, at: (f64, f64)) {
    p.app
        .observe_net_hover(Some(SelectedItem::new(id(uuid), kind)), at);
    if let Some(c) = p.app.document_state.net_nav.hover.candidate.as_mut() {
        c.since = Instant::now()
            .checked_sub(DWELL + Duration::from_millis(50))
            .expect("clock");
    }
    p.send_settled(NetNavMsg::HoverDwellTicked);
}

fn card(p: &Project) -> crate::app::state::net_nav::HoverCard {
    p.app
        .document_state
        .net_nav
        .hover
        .card
        .clone()
        .expect("a hover card")
}

fn pins(card: &crate::app::state::net_nav::HoverCard) -> Vec<(String, String)> {
    card.pins
        .iter()
        .map(|p| (p.pin.clone(), p.sheet.clone()))
        .collect()
}

fn all_vcc_pins() -> Vec<(String, String)> {
    [("R1.1", "root"), ("R2.1", "mid"), ("R3.1", "third")]
        .map(|(pin, sheet)| (pin.to_string(), sheet.to_string()))
        .to_vec()
}

#[test]
fn a_wire_shows_its_net_and_every_pin_with_its_sheet() {
    let mut p = project();
    dwell_on(&mut p, ROOT_VCC_WIRE, SelectedKind::Wire, (5.0, 0.0));
    let c = card(&p);
    assert_eq!(c.name, "VCC");
    assert_eq!(c.class, None);
    assert_eq!(pins(&c), all_vcc_pins());
}

#[test]
fn a_pin_shows_the_net_it_is_on() {
    let mut p = project();
    dwell_on(&mut p, ROOT_R1, SelectedKind::Symbol, (0.2, 0.1));
    let c = card(&p);
    assert_eq!(c.name, "VCC");
    assert_eq!(pins(&c), all_vcc_pins());
}

#[test]
fn a_label_shows_its_net_and_class() {
    let mut p = project();
    let id = p.app.document_state.projects[0].id;
    p.app
        .document_state
        .project_by_id_mut(id)
        .expect("project")
        .data
        .net_classes = vec![NetClass {
        nets: vec!["VCC".to_string()],
        ..NetClass::named("Power")
    }];
    p.focus(&p.mid.clone());
    dwell_on(&mut p, MID_VCC, SelectedKind::Label, (0.0, 0.0));
    let c = card(&p);
    assert_eq!(c.name, "VCC");
    assert_eq!(c.class.as_deref(), Some("Power"));
}

#[test]
fn nothing_shows_before_the_dwell_is_up() {
    let mut p = project();
    p.app.observe_net_hover(
        Some(SelectedItem::new(id(ROOT_VCC_WIRE), SelectedKind::Wire)),
        (5.0, 0.0),
    );
    p.send_settled(NetNavMsg::HoverDwellTicked);
    assert!(p.app.document_state.net_nav.hover.card.is_none());
    assert!(p.app.document_state.net_nav.hover.needs_ticks());
}

#[test]
fn with_the_preference_off_there_is_no_hover_and_no_timer() {
    let mut p = project();
    p.app.ui_state.net_hover_enabled = false;
    p.app.observe_net_hover(
        Some(SelectedItem::new(id(ROOT_VCC_WIRE), SelectedKind::Wire)),
        (5.0, 0.0),
    );
    assert!(p.app.document_state.net_nav.hover.candidate.is_none());
    assert!(!p.app.document_state.net_nav.hover.needs_ticks());
    p.send_settled(NetNavMsg::HoverDwellTicked);
    assert!(p.app.document_state.net_nav.hover.card.is_none());
}

#[test]
fn the_preference_defaults_on_and_the_card_leaves_with_the_cursor() {
    let mut p = project();
    assert!(p.app.ui_state.net_hover_enabled);
    dwell_on(&mut p, ROOT_VCC_WIRE, SelectedKind::Wire, (5.0, 0.0));
    p.app.observe_net_hover(None, (50.0, 50.0));
    assert!(p.app.document_state.net_nav.hover.card.is_none());
}

#[test]
fn a_second_dwell_reads_the_cached_index_without_a_rebuild() {
    let mut p = project();
    dwell_on(&mut p, ROOT_VCC_WIRE, SelectedKind::Wire, (5.0, 0.0));
    // No settle: a rebuild would leave this dwell unanswered.
    p.focus(&p.mid.clone());
    p.app.observe_net_hover(
        Some(SelectedItem::new(id(MID_VCC), SelectedKind::Label)),
        (0.0, 0.0),
    );
    p.app
        .document_state
        .net_nav
        .hover
        .candidate
        .as_mut()
        .expect("candidate")
        .since = Instant::now().checked_sub(DWELL * 2).expect("clock");
    p.send(NetNavMsg::HoverDwellTicked);
    assert_eq!(card(&p).name, "VCC");
}

#[test]
fn opening_the_inline_editor_clears_the_card_and_blocks_a_new_one() {
    let mut p = project();
    p.focus(&p.mid.clone());
    dwell_on(&mut p, MID_VCC, SelectedKind::Label, (0.0, 0.0));
    assert!(p.app.document_state.net_nav.hover.card.is_some());

    assert!(
        p.app
            .begin_text_edit(SelectedItem::new(id(MID_VCC), SelectedKind::Label))
    );
    assert!(p.app.document_state.net_nav.hover.card.is_none());

    dwell_on(&mut p, MID_VCC, SelectedKind::Label, (0.0, 0.0));
    assert!(
        p.app.document_state.net_nav.hover.card.is_none(),
        "a card must not appear over the open editor"
    );
}

#[test]
fn the_card_can_come_back_on_the_same_label_after_the_editor_closes() {
    let mut p = project();
    p.focus(&p.mid.clone());
    dwell_on(&mut p, MID_VCC, SelectedKind::Label, (0.0, 0.0));
    assert!(
        p.app
            .begin_text_edit(SelectedItem::new(id(MID_VCC), SelectedKind::Label))
    );
    assert!(p.app.document_state.net_nav.hover.candidate.is_none());

    // Esc: the editor closes and the mouse has not moved.
    p.app.interaction_state.editing_text = None;
    dwell_on(&mut p, MID_VCC, SelectedKind::Label, (0.0, 0.0));
    assert!(p.app.document_state.net_nav.hover.card.is_some());
}
