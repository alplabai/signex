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

fn highlight_name(p: &Project) -> Option<String> {
    p.app
        .document_state
        .net_nav
        .highlight
        .as_ref()
        .map(|h| h.name.clone())
}

/// The child opened straight from disk (no recorded placement) shows the
/// smallest placement, as the designators do; here that is A while B's nets
/// sort first.
#[test]
fn a_directly_opened_tab_highlights_in_the_placement_the_designators_show() {
    let (mut p, child) = placed_twice_b_first();
    p.app.open_sheet_with_placement(child, None);
    p.select(TWICE_X, SelectedKind::Label);

    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);

    assert!(
        highlight_name(&p).is_some_and(|n| n.ends_with("/X")),
        "{:?}",
        highlight_name(&p)
    );
    assert!(
        p.bright().is_some_and(|s| s.contains(&id(TWICE_X))),
        "the label just highlighted is bright, not dimmed"
    );
}

#[test]
fn a_net_only_b_reaches_is_dim_on_a_directly_opened_tab_showing_a() {
    let (mut p, child) = placed_twice_b_first();
    // B/SIG, started from B's sheet entry on root.
    p.select(TWICE_ENTRY_B, SelectedKind::SheetPin);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    assert_eq!(highlight_name(&p).as_deref(), Some("B/SIG"));

    p.app.open_sheet_with_placement(child, None);

    assert!(
        p.bright().is_some_and(|s| s.is_empty()),
        "the tab shows A (the smallest placement); B/SIG is not there"
    );
}

#[test]
fn a_recorded_placement_whose_sheet_symbol_is_gone_follows_the_default() {
    let (mut p, child) = placed_twice();
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_B)));
    p.focus(&p.root.clone());
    p.select(TWICE_B, SelectedKind::ChildSheet);
    let _task = p.app.update(crate::app::Message::Edit(
        crate::app::EditMsg::DeleteSelected,
    ));
    p.focus(&child); // the tab still records the placement that no longer exists
    p.select(TWICE_X, SelectedKind::Label);

    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);

    assert!(
        highlight_name(&p).is_some_and(|n| n.starts_with("A/")),
        "{:?}",
        highlight_name(&p)
    );
    assert!(p.bright().is_some_and(|s| s.contains(&id(TWICE_X))));
}

#[test]
fn renaming_the_highlighted_net_keeps_it_on_its_own_placement() {
    let (mut p, child) = placed_twice();
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_B)));
    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);

    // Rename the child's port: both placements' nets change name.
    let rename = crate::panels::PanelMsg::EditLabelText(id(TWICE_LABEL), "SIGX".to_string());
    let _task = p
        .app
        .update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
            rename,
        )));
    p.app.settle_net_index_blocking();

    assert!(
        highlight_name(&p).is_some_and(|n| n.starts_with("B/")),
        "{:?}",
        highlight_name(&p)
    );
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_A)));
    assert!(
        p.bright().is_some_and(|s| s.is_empty()),
        "A's renamed net is not the highlight"
    );
    p.app.open_sheet_with_placement(child, Some(via(TWICE_B)));
    assert!(p.bright().is_some_and(|s| s.contains(&id(TWICE_LABEL))));
}

#[test]
fn a_partner_jump_lands_in_the_placement_of_its_own_net() {
    let (mut p, child) = placed_twice();
    p.select(TWICE_ENTRY_A, SelectedKind::SheetPin);
    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert_eq!(p.active(), Some(child.clone()));
    assert_eq!(
        p.app.document_state.tab_placements.get(&child),
        Some(&via(TWICE_A))
    );

    p.focus(&p.root.clone());
    p.select(TWICE_ENTRY_B, SelectedKind::SheetPin);
    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert_eq!(
        p.app.document_state.tab_placements.get(&child),
        Some(&via(TWICE_B))
    );
    assert_eq!(p.selected().first().map(|i| i.uuid), Some(id(TWICE_LABEL)));
}

#[test]
fn a_lap_does_not_carry_over_to_another_placement_of_the_same_file() {
    let (mut p, child) = placed_twice();
    p.app
        .open_sheet_with_placement(child.clone(), Some(via(TWICE_B)));
    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::PartnerJumpRequested); // to B's entry on root
    p.send_settled(NetNavMsg::PartnerJumpRequested); // back to the label (child via B)
    assert_eq!(p.active(), Some(child.clone()));

    p.app.open_sheet_with_placement(child, Some(via(TWICE_A)));
    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::PartnerJumpRequested);

    assert_eq!(
        p.selected().first().map(|i| i.uuid),
        Some(id(TWICE_ENTRY_A)),
        "A's own entry"
    );
}
