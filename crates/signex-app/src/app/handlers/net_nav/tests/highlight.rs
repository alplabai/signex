//! The persistent net highlight (#1271).

use signex_types::schematic::SelectedKind;

use super::fixture::*;
use crate::app::NetNavMsg;

/// Highlight the VCC net from root's label.
fn highlighted() -> Project {
    let mut p = project();
    p.select(ROOT_VCC, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    p
}

#[test]
fn the_net_stays_bright_on_every_sheet_it_reaches_and_the_rest_dims() {
    let mut p = highlighted();
    assert_eq!(
        p.app
            .document_state
            .net_nav
            .highlight
            .as_ref()
            .map(|h| h.name.as_str()),
        Some("VCC")
    );
    let root = p.bright().expect("highlight on root");
    assert!(root.contains(&id(ROOT_VCC_WIRE)) && root.contains(&id(ROOT_R1)));

    p.focus(&p.mid.clone());
    let mid = p.bright().expect("highlight survives the tab switch");
    assert!(mid.contains(&id(MID_VCC)) && mid.contains(&id(MID_R2)));
    assert!(!mid.contains(&id(ALONE)), "another net on the sheet dims");

    p.focus(&p.third.clone());
    assert!(p.bright().expect("third").contains(&id(THIRD_R3)));
}

#[test]
fn a_sheet_the_net_does_not_reach_shows_everything_dimmed() {
    let mut p = project();
    p.focus(&p.mid.clone());
    p.select(ALONE, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    assert!(p.bright().expect("mid").contains(&id(ALONE)));

    p.focus(&p.third.clone());

    assert_eq!(p.bright().map(|s| s.is_empty()), Some(true));
}

#[test]
fn related_highlight_selects_the_members_on_this_sheet() {
    let p = highlighted();
    let picked: std::collections::HashSet<_> = p.selected().iter().map(|i| i.uuid).collect();
    assert!(picked.contains(&id(ROOT_VCC_WIRE)));
    assert!(picked.contains(&id(ROOT_R1)));
    assert!(picked.contains(&id(ROOT_VCC)));
}

#[test]
fn next_steps_through_the_members_across_sheets_and_wraps() {
    let mut p = highlighted();
    let mut seen = Vec::new();
    for _ in 0..7 {
        p.send_settled(NetNavMsg::HighlightStepRequested { forward: true });
        let on = p.active().expect("sheet");
        let item = p.selected().first().map(|i| i.uuid).expect("selection");
        seen.push((on, item));
    }
    let (root, mid, third) = (p.root.clone(), p.mid.clone(), p.third.clone());
    assert_eq!(
        seen,
        [
            (root.clone(), id(ROOT_R1)),
            (root, id(ROOT_VCC)),
            (mid.clone(), id(MID_R2)),
            (mid, id(MID_VCC)),
            (third.clone(), id(THIRD_R3)),
            (third, id(THIRD_VCC)),
            (p.root.clone(), id(ROOT_R1)),
        ]
    );
}

#[test]
fn previous_starts_from_the_last_member() {
    let mut p = highlighted();
    p.send_settled(NetNavMsg::HighlightStepRequested { forward: false });
    assert_eq!(p.active(), Some(p.third.clone()));
    assert_eq!(p.selected().first().map(|i| i.uuid), Some(id(THIRD_VCC)));
}

#[test]
fn clearing_removes_the_highlight_from_the_model_and_every_canvas() {
    let mut p = highlighted();
    p.send(NetNavMsg::HighlightClearRequested);
    assert!(p.app.document_state.net_nav.highlight.is_none());
    assert!(p.bright().is_none());
    p.focus(&p.mid.clone());
    assert!(p.bright().is_none(), "still clear on another tab");
}

#[test]
fn stepping_with_nothing_highlighted_says_how_to_start() {
    let mut p = project();
    p.send(NetNavMsg::HighlightStepRequested { forward: true });
    assert!(logged(&["none is highlighted"]));
    p.send(NetNavMsg::HighlightClearRequested);
    assert!(logged(&["Nothing to clear"]));
}

#[test]
fn a_cleared_highlight_after_an_edit_that_removes_the_net_says_so() {
    let mut p = highlighted();
    // The anchor and the name are gone from a rebuilt index.
    p.app
        .document_state
        .net_nav
        .highlight
        .as_mut()
        .expect("highlight")
        .name = "GONE".to_string();
    p.app
        .document_state
        .net_nav
        .highlight
        .as_mut()
        .expect("highlight")
        .anchor = (signex_net::SheetKey::new("nowhere"), id(404));
    p.send_settled(NetNavMsg::HighlightStepRequested { forward: true });
    assert!(p.app.document_state.net_nav.highlight.is_none());
    assert!(p.bright().is_none());
}
