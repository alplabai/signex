//! m7 panel list, m8 dock drag zones.

use signex_app::app::{Message, OverlayMsg, Signex, UiMsg};
use signex_app::dock::{DockMessage, DockTabAction, PanelPosition};
use signex_app::panels::PanelKind;
use signex_app::panels::panel_groups::grouped_panels;

use super::support::{Desk, desk, esc, is_dirty, main_window};

const REGIONS: [PanelPosition; 3] = [
    PanelPosition::Left,
    PanelPosition::Right,
    PanelPosition::Bottom,
];

fn dock(app: &mut Signex, msg: DockMessage) {
    let _ = app.update(Message::Dock(msg));
}

/// The region and tab index holding `kind`.
fn site(app: &Signex, kind: PanelKind) -> Option<(PanelPosition, usize)> {
    REGIONS.into_iter().find_map(|region| {
        app.document_state
            .dock
            .panel_kinds(region)
            .iter()
            .position(|k| *k == kind)
            .map(|index| (region, index))
    })
}

fn copies(app: &Signex, kind: PanelKind) -> usize {
    REGIONS
        .into_iter()
        .map(|region| {
            app.document_state
                .dock
                .panel_kinds(region)
                .iter()
                .filter(|k| **k == kind)
                .count()
        })
        .sum()
}

/// A sheet open, with Projects docked on the left.
fn docked() -> (Desk, usize) {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    desk.app
        .document_state
        .dock
        .show_panel_at(PanelPosition::Left, PanelKind::Projects);
    let (region, index) = site(&desk.app, PanelKind::Projects).expect("Projects is docked");
    assert_eq!(region, PanelPosition::Left);
    (desk, index)
}

// ---- m7 Panel list --------------------------------------------------------

#[test]
fn m7_picking_a_closed_panel_docks_it_and_closes_the_list() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    desk.app
        .document_state
        .dock
        .close_panel(PanelKind::Messages);
    let _ = desk
        .app
        .update(Message::Overlay(OverlayMsg::TogglePanelList));
    assert!(desk.app.ui_state.panel_list_open);

    let _ = desk
        .app
        .update(Message::Overlay(OverlayMsg::OpenPanel(PanelKind::Messages)));

    assert!(!desk.app.ui_state.panel_list_open);
    assert_eq!(copies(&desk.app, PanelKind::Messages), 1);
    assert!(
        desk.app
            .document_state
            .dock
            .is_frontmost(PanelKind::Messages)
    );
}

#[test]
fn m7_picking_an_open_panel_focuses_it_without_a_second_copy() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    let dock = &mut desk.app.document_state.dock;
    dock.show_panel_at(PanelPosition::Right, PanelKind::Properties);
    dock.show_panel_at(PanelPosition::Right, PanelKind::Filter);
    assert!(!dock.is_frontmost(PanelKind::Properties));

    let _ = desk.app.update(Message::Overlay(OverlayMsg::OpenPanel(
        PanelKind::Properties,
    )));

    assert_eq!(copies(&desk.app, PanelKind::Properties), 1);
    assert!(
        desk.app
            .document_state
            .dock
            .is_frontmost(PanelKind::Properties)
    );
}

#[test]
fn m7_closing_the_list_without_a_pick_changes_no_dock() {
    let (mut desk, _) = docked();
    let before: Vec<Vec<PanelKind>> = REGIONS
        .iter()
        .map(|r| desk.app.document_state.dock.panel_kinds(*r).to_vec())
        .collect();
    let _ = desk
        .app
        .update(Message::Overlay(OverlayMsg::TogglePanelList));
    let _ = desk
        .app
        .update(Message::Overlay(OverlayMsg::TogglePanelList));
    let after: Vec<Vec<PanelKind>> = REGIONS
        .iter()
        .map(|r| desk.app.document_state.dock.panel_kinds(*r).to_vec())
        .collect();
    assert_eq!(before, after);
}

/// Row m7 and the 2026-11-18 decision: these are document tabs, not panels,
/// so the list does not offer them.
#[test]
fn m7_the_list_does_not_offer_the_tools_that_are_document_tabs() {
    let listed: Vec<PanelKind> = grouped_panels(|_| true)
        .into_iter()
        .flat_map(|(_, kinds)| kinds)
        .collect();
    for tool in [
        PanelKind::BomStudio,
        PanelKind::OutputJobs,
        PanelKind::LayerStack,
        PanelKind::Variants,
    ] {
        assert!(!listed.contains(&tool), "{tool:?} is still in the list");
    }
}

#[test]
fn m7_a_group_with_no_panel_left_is_not_shown() {
    let groups = grouped_panels(|kind| kind == PanelKind::Messages);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].1, [PanelKind::Messages]);
}

// ---- m8 Dock drag zones ---------------------------------------------------

#[test]
fn m8_float_from_the_tab_menu_leaves_the_dock_and_rests() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::TabContextMenuRequested(PanelPosition::Left, index),
    );
    assert!(desk.app.document_state.dock.tab_menu.is_some());

    dock(
        &mut desk.app,
        DockMessage::TabMenuActionPicked(DockTabAction::Float),
    );

    assert!(desk.app.document_state.dock.tab_menu.is_none());
    assert_eq!(site(&desk.app, PanelKind::Projects), None);
    let floating = &desk.app.document_state.dock.floating;
    assert_eq!(floating.len(), 1);
    assert_eq!(floating[0].kind, PanelKind::Projects);
    assert!(!floating[0].dragging, "a menu pick starts no drag");
}

#[test]
fn m8_dock_right_from_the_tab_menu_moves_the_panel() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::TabContextMenuRequested(PanelPosition::Left, index),
    );
    dock(
        &mut desk.app,
        DockMessage::TabMenuActionPicked(DockTabAction::MoveTo(PanelPosition::Right)),
    );
    let (region, _) = site(&desk.app, PanelKind::Projects).expect("still docked");
    assert_eq!(region, PanelPosition::Right);
    assert_eq!(copies(&desk.app, PanelKind::Projects), 1);
}

#[test]
fn m8_close_from_the_tab_menu_removes_the_panel() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::TabContextMenuRequested(PanelPosition::Left, index),
    );
    dock(
        &mut desk.app,
        DockMessage::TabMenuActionPicked(DockTabAction::Close),
    );
    assert_eq!(copies(&desk.app, PanelKind::Projects), 0);
    assert!(desk.app.document_state.dock.floating.is_empty());
}

#[test]
fn m8_dismissing_the_tab_menu_leaves_the_panel_where_it_was() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::TabContextMenuRequested(PanelPosition::Left, index),
    );
    dock(&mut desk.app, DockMessage::TabMenuDismissed);
    assert!(desk.app.document_state.dock.tab_menu.is_none());
    assert_eq!(
        site(&desk.app, PanelKind::Projects),
        Some((PanelPosition::Left, index))
    );
}

#[test]
fn m8_a_drag_to_the_right_edge_previews_and_docks_there_on_release() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::UndockPanel(PanelPosition::Left, index),
    );
    let _ = desk.app.update(Message::Ui(UiMsg::DragMove(1390.0, 450.0)));
    assert_eq!(
        desk.app.document_state.dock.drop_preview,
        Some(PanelPosition::Right)
    );

    let main = main_window(&desk.app);
    let _ = desk.app.update(Message::Ui(UiMsg::DragEnd(main)));

    assert_eq!(desk.app.document_state.dock.drop_preview, None);
    assert!(desk.app.document_state.dock.floating.is_empty());
    let (region, _) = site(&desk.app, PanelKind::Projects).expect("docked");
    assert_eq!(region, PanelPosition::Right);
}

#[test]
fn m8_a_release_away_from_every_zone_leaves_the_panel_floating() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::UndockPanel(PanelPosition::Left, index),
    );
    let _ = desk.app.update(Message::Ui(UiMsg::DragMove(700.0, 300.0)));
    assert_eq!(desk.app.document_state.dock.drop_preview, None);

    let main = main_window(&desk.app);
    let _ = desk.app.update(Message::Ui(UiMsg::DragEnd(main)));

    let floating = &desk.app.document_state.dock.floating;
    assert_eq!(floating.len(), 1);
    assert!(!floating[0].dragging);
    assert_eq!(site(&desk.app, PanelKind::Projects), None);
}

#[test]
fn m8_esc_during_a_drag_cancels_the_preview_and_keeps_the_panel_floating() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::UndockPanel(PanelPosition::Left, index),
    );
    let _ = desk.app.update(Message::Ui(UiMsg::DragMove(1390.0, 450.0)));
    assert!(desk.app.document_state.dock.drop_preview.is_some());

    esc(&mut desk.app);

    let dock_state = &desk.app.document_state.dock;
    assert_eq!(dock_state.drop_preview, None);
    assert_eq!(dock_state.floating.len(), 1);
    assert!(!dock_state.floating[0].dragging);
}

/// The 2026-11-20 decision (h): Dock back goes to the home region, not the
/// last region the panel was in.
#[test]
fn m8_dock_back_returns_a_panel_to_its_home_region() {
    let (mut desk, index) = docked();
    dock(
        &mut desk.app,
        DockMessage::TabContextMenuRequested(PanelPosition::Left, index),
    );
    dock(
        &mut desk.app,
        DockMessage::TabMenuActionPicked(DockTabAction::MoveTo(PanelPosition::Bottom)),
    );
    let (region, index) = site(&desk.app, PanelKind::Projects).expect("docked");
    assert_eq!(region, PanelPosition::Bottom);
    dock(&mut desk.app, DockMessage::UndockPanel(region, index));

    dock(&mut desk.app, DockMessage::DockFloating(0));

    assert!(desk.app.document_state.dock.floating.is_empty());
    let (home, _) = site(&desk.app, PanelKind::Projects).expect("docked");
    assert_eq!(home, PanelPosition::Left);
}

#[test]
fn m8_docking_a_panel_does_not_dirty_the_open_sheet() {
    let (mut desk, index) = docked();
    let path = desk.path("a.snxsch");
    dock(
        &mut desk.app,
        DockMessage::UndockPanel(PanelPosition::Left, index),
    );
    dock(&mut desk.app, DockMessage::DockFloating(0));
    assert!(!is_dirty(&desk.app, &path));
}
