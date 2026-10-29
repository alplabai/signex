//! The highlight and stepping follow the placement a sheet is shown through.

use signex_types::schematic::{InstancePath, SelectedKind};

use super::fixture::*;
use crate::app::NetNavMsg;

fn via(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![id(n)])
}

#[test]
fn the_highlight_is_bright_only_in_the_placement_it_was_made_in() {
    let (mut p, child) = placed_twice();
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_B)));
    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    assert!(
        p.bright().is_some_and(|s| s.contains(&id(TWICE_LABEL))),
        "bright in B"
    );

    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_A)));
    assert!(
        p.bright().is_some_and(|s| s.is_empty()),
        "A's copy is on A/SIG, not B/SIG: everything dims"
    );

    p.app.open_sheet_with_placement(child, Some(via(TWICE_B)));
    assert!(
        p.bright().is_some_and(|s| s.contains(&id(TWICE_LABEL))),
        "bright again in B"
    );
}

#[test]
fn stepping_to_a_member_of_another_placement_switches_to_it() {
    let (mut p, child) = placed_twice();
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_B)));
    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_A)));

    // The last member of B/SIG is the label on the child, in placement B.
    p.send_settled(NetNavMsg::HighlightStepRequested { forward: false });

    assert_eq!(
        p.app.document_state.tab_placements.get(&child),
        Some(&via(TWICE_B))
    );
    assert_eq!(p.selected().first().map(|i| i.uuid), Some(id(TWICE_LABEL)));
}

#[test]
fn the_highlight_stays_on_its_own_placements_net_after_an_edit() {
    let (mut p, child) = placed_twice();
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_B)));
    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);

    // A real edit on the child's SIG wire rebuilds the index.
    p.select(811, SelectedKind::Wire);
    let _task = p.app.update(crate::app::Message::Edit(
        crate::app::EditMsg::DeleteSelected,
    ));
    p.app.settle_net_index_blocking();

    let name = p
        .app
        .document_state
        .net_nav
        .highlight
        .as_ref()
        .map(|h| h.name.clone());
    assert!(name.is_some_and(|n| n.starts_with("B/")), "still B's net");
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_A)));
    assert!(p.bright().is_some_and(|s| s.is_empty()), "A stays dim");
    p.app.open_sheet_with_placement(child, Some(via(TWICE_B)));
    assert!(p.bright().is_some_and(|s| s.contains(&id(TWICE_LABEL))));
}
