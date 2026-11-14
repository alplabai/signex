//! #1966: an undocked window's descent into a sheet symbol, by Enter or a
//! double click, and the project-wide steps that open a sheet, all run on
//! the main window.

use super::*;

#[test]
fn enter_on_a_sheet_symbol_in_a_window_descends_on_the_main_window_child_open() {
    let mut d = descent_fixture(true);
    enter_in_window(&mut d);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
}

#[test]
fn enter_on_a_sheet_symbol_in_a_window_descends_on_the_main_window_child_not_open() {
    let mut d = descent_fixture(false);
    let command = crate::keymap::AppCommandId::new("placement_accept").unwrap();
    let (_outcome, task) = d.app.dispatch_command_reporting(
        &command,
        crate::app::command::CommandArgs::none(),
        Some(d.window),
    );
    // Nothing was read for the window: no descent waits with the window's
    // sheet as its parent.
    assert!(d.app.ui_state.pending_sheet_opens.is_empty());
    crate::app::path_ops::settle_task(&mut d.app, task);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
    assert!(d.app.ui_state.pending_sheet_opens.is_empty());
}

#[test]
fn a_double_click_on_a_sheet_symbol_in_a_window_descends_on_the_main_window() {
    let mut d = descent_fixture(true);
    // The window's sheet was itself entered through a placement; main's
    // sheet has none, so only the window's can explain the child's.
    let parent = signex_types::schematic::InstancePath::root().child(uuid::Uuid::from_u128(500));
    d.app
        .document_state
        .tab_placements
        .record(&d.top, Some(parent.clone()));
    let task = d.app.update(Message::CanvasEventInWindow {
        window_id: d.window,
        event: crate::canvas::CanvasEvent::DoubleClicked {
            world_x: 5.0,
            world_y: 5.0,
            screen_x: 0.0,
            screen_y: 0.0,
        },
    });
    crate::app::path_ops::settle_task(&mut d.app, task);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
    // The child is shown through the placement of the symbol that was
    // double-clicked on the window's sheet, not one derived from main's.
    let expected = parent.child(uuid::Uuid::from_u128(1));
    assert_eq!(
        d.app.document_state.tab_placements.get(&d.child),
        Some(&expected)
    );
}

#[test]
fn focus_open_sheet_refuses_inside_a_window_swap_and_says_so() {
    let _ = crate::diagnostics::init_logging();
    let mut d = descent_fixture(true);
    d.app.interaction_state.acting_window = Some(d.window);
    let before = d.app.document_state.active_path.clone();
    assert!(!d.app.focus_open_sheet(&d.child));
    assert_eq!(d.app.document_state.active_path, before);
    assert!(said("was not done from inside an undocked window"));
    d.app.interaction_state.acting_window = None;
    assert!(d.app.focus_open_sheet(&d.child));
}

#[test]
fn find_next_with_project_scope_from_a_window_opens_the_hit_on_the_main_window() {
    let mut d = descent_fixture(false);
    let hit = signex_engine::FindHit {
        kind: signex_engine::FindKind::NetLabel,
        item: SelectedItem::new(uuid::Uuid::from_u128(77), SelectedKind::Label),
        target: signex_engine::TextTarget::Label(uuid::Uuid::from_u128(77)),
        text: "CHILD_NET".into(),
        placement: None,
        single: true,
        position: Point::new(1.0, 1.0),
    };
    let find = &mut d.app.ui_state.find_replace;
    find.scope = crate::find_replace::FindScope::Project;
    find.matches = vec![crate::find_replace::FindMatch {
        sheet: Some(d.child.clone()),
        hit,
        placement_label: None,
    }];
    let command = crate::keymap::AppCommandId::new("find_next").unwrap();
    let (_outcome, task) = d.app.dispatch_command_reporting(
        &command,
        crate::app::command::CommandArgs::none(),
        Some(d.window),
    );
    crate::app::path_ops::settle_task(&mut d.app, task);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
}

#[test]
fn a_descent_from_a_window_reaches_an_open_child_that_is_not_on_disk() {
    let mut d = descent_fixture(true);
    std::fs::remove_file(&d.child).unwrap();
    enter_in_window(&mut d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
}
