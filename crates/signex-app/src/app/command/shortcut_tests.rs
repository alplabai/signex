//! Panel and document-tab shortcuts through the registry (#103).

use std::path::PathBuf;

use super::*;
use crate::app::state::WindowKind;
use crate::keymap::AppCommandId;
use crate::menu_bar::panel_check_tests::{context, view_row_checked};
use crate::panels::PanelKind;

fn run(app: &mut Signex, id: &str) {
    let command = AppCommandId::new(id).unwrap();
    let message = bridge::core_to_message(&command).expect("the command resolves");
    let _ = app.update(message);
}

/// A fresh app with the Properties panel closed, so each test starts from
/// a known state whatever the boot layout is.
fn app_without_properties() -> Signex {
    let (mut app, _task) = Signex::new();
    app.document_state.dock.close_panel(PanelKind::Properties);
    assert!(!app.is_panel_open(PanelKind::Properties));
    app
}

fn in_front(app: &Signex) -> bool {
    app.document_state.dock.is_frontmost(PanelKind::Properties)
}

#[test]
fn panel_shortcut_opens_a_closed_panel() {
    let mut app = app_without_properties();

    run(&mut app, "toggle_properties_panel");

    assert!(app.is_panel_open(PanelKind::Properties), "check mark on");
    assert!(in_front(&app));
}

#[test]
fn panel_shortcut_closes_an_open_focused_panel() {
    let mut app = app_without_properties();
    run(&mut app, "toggle_properties_panel");

    run(&mut app, "toggle_properties_panel");

    assert!(!app.is_panel_open(PanelKind::Properties), "check mark off");
    assert!(!app.document_state.dock.is_frontmost(PanelKind::Properties));
}

#[test]
fn panel_shortcut_focuses_an_open_panel_hidden_behind_another_tab() {
    let mut app = app_without_properties();
    run(&mut app, "toggle_properties_panel");
    // Same home region as Properties; showing it takes the front.
    app.document_state.dock.show_panel(PanelKind::Filter);
    assert!(app.is_panel_open(PanelKind::Properties));
    assert!(!in_front(&app), "fixture: Properties is behind Filter");

    run(&mut app, "toggle_properties_panel");

    assert!(app.is_panel_open(PanelKind::Properties), "still open");
    assert!(in_front(&app), "brought to the front");
}

#[test]
fn panel_shortcut_focuses_an_open_panel_in_a_collapsed_region() {
    let mut app = app_without_properties();
    run(&mut app, "toggle_properties_panel");
    let home = crate::dock::PanelPosition::default_for(PanelKind::Properties);
    app.document_state
        .dock
        .update(crate::dock::DockMessage::ToggleCollapse(home));
    assert!(!in_front(&app), "fixture: the region is collapsed");

    run(&mut app, "toggle_properties_panel");

    assert!(app.is_panel_open(PanelKind::Properties));
    assert!(in_front(&app));
}

#[test]
fn every_panel_shortcut_toggles_its_own_panel() {
    for (id, kind) in [
        ("toggle_properties_panel", PanelKind::Properties),
        ("toggle_schematic_filter_panel", PanelKind::SchFilter),
        ("toggle_schematic_list_panel", PanelKind::SchList),
    ] {
        let (mut app, _task) = Signex::new();
        app.document_state.dock.close_panel(kind);
        run(&mut app, id);
        assert!(app.is_panel_open(kind), "{id} opens {kind:?}");
        run(&mut app, id);
        assert!(!app.is_panel_open(kind), "{id} closes {kind:?}");
    }
}

fn app_with_tabs(kinds: Vec<TabKind>) -> Signex {
    let (mut app, _task) = Signex::new();
    for (i, kind) in kinds.into_iter().enumerate() {
        app.document_state.tabs.push(crate::app::TabInfo {
            title: format!("t{i}"),
            path: PathBuf::from(format!("t{i}.snxsch")),
            cached_document: None,
            project_id: None,
            kind,
        });
    }
    app.document_state.active_tab = 0;
    app
}

fn mixed_tabs() -> Signex {
    app_with_tabs(vec![
        TabKind::Schematic,
        TabKind::LibraryBrowser(PathBuf::from("a.snxlib")),
        TabKind::Pcb,
        TabKind::SymbolEditor(PathBuf::from("s.snxsym")),
        TabKind::FootprintEditor(PathBuf::from("f.snxfpt")),
    ])
}

#[test]
fn next_tab_visits_every_kind_in_tab_bar_order_and_wraps_to_the_first() {
    let mut app = mixed_tabs();
    for expected in [1, 2, 3, 4, 0] {
        run(&mut app, "next_document_tab");
        assert_eq!(app.document_state.active_tab, expected);
    }
}

#[test]
fn next_tab_from_a_schematic_reaches_the_following_library_tab() {
    let mut app = mixed_tabs();
    run(&mut app, "next_document_tab");
    assert!(matches!(
        app.document_state.tabs[app.document_state.active_tab].kind,
        TabKind::LibraryBrowser(_)
    ));
}

#[test]
fn next_tab_from_a_schematic_reaches_a_following_pcb_tab() {
    let mut app = app_with_tabs(vec![TabKind::Schematic, TabKind::Pcb]);
    run(&mut app, "next_document_tab");
    assert_eq!(app.document_state.active_tab, 1);
}

#[test]
fn previous_tab_goes_backwards_and_wraps_to_the_last() {
    let mut app = mixed_tabs();
    run(&mut app, "previous_document_tab");
    assert_eq!(app.document_state.active_tab, 4, "wraps to the last tab");
    for expected in [3, 2, 1, 0] {
        run(&mut app, "previous_document_tab");
        assert_eq!(app.document_state.active_tab, expected);
    }
}

/// #103 - a panel in a window of its own counts as open, so its View-menu
/// row stays checked.
#[test]
fn the_menu_check_counts_a_detached_panel_window_counts_as_open() {
    let (mut app, _task) = Signex::new();
    app.document_state.dock.close_panel(PanelKind::Messages);
    assert_eq!(view_row_checked(&context(&app), "Messages"), Some(false));

    app.ui_state.windows.insert(
        iced::window::Id::unique(),
        WindowKind::DetachedPanel(PanelKind::Messages),
    );

    assert_eq!(view_row_checked(&context(&app), "Messages"), Some(true));
}
