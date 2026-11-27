//! Mockup rows 119-121 (alias rows kept by Caner, 2026-11-20): the SCH
//! Library and Footprint Library rows of the panel list and the command palette are aliases that open the Library
//! panel in their kind; neither is ever docked itself.

use crate::app::Signex;
use crate::library::state::LibraryPanelKind;
use crate::panels::PanelKind;

fn app() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.document_view.dock = crate::dock::graphite_first_run();
    app
}

#[test]
fn an_alias_row_opens_the_library_panel_in_its_kind() {
    for (alias, kind) in [
        (PanelKind::SchLibrary, LibraryPanelKind::Symbols),
        (PanelKind::FootprintLibrary, LibraryPanelKind::Footprints),
    ] {
        let mut app = app();
        let _opened = app.show_panel(alias);
        assert!(
            app.document_view.dock.is_frontmost(PanelKind::Library),
            "{alias:?} opens the Library panel"
        );
        assert!(
            app.document_view.dock.locate(alias).is_none(),
            "{alias:?} is never docked"
        );
        assert_eq!(app.library_panel_kind(), kind, "{alias:?}");
        assert!(app.is_panel_open(alias), "{alias:?} shows checked");
        assert!(app.is_panel_open(PanelKind::Library));
    }
}

#[test]
fn an_alias_row_is_checked_only_while_the_library_panel_shows_its_kind() {
    let mut app = app();
    assert!(!app.is_panel_open(PanelKind::SchLibrary));
    let _opened = app.show_panel(PanelKind::Library);
    // Outside the editors the panel opens on Parts.
    assert_eq!(app.library_panel_kind(), LibraryPanelKind::Parts);
    assert!(!app.is_panel_open(PanelKind::SchLibrary));
    assert!(!app.is_panel_open(PanelKind::FootprintLibrary));
    assert!(
        !app.open_panels().contains(&PanelKind::SchLibrary),
        "the panel list leaves SCH Library unchecked"
    );
}

#[test]
fn toggling_an_alias_switches_the_kind_first_and_closes_the_panel_second() {
    let mut app = app();
    let _opened = app.show_panel(PanelKind::SchLibrary);
    let _toggled = app.toggle_panel(PanelKind::FootprintLibrary);
    assert_eq!(app.library_panel_kind(), LibraryPanelKind::Footprints);
    assert!(
        app.is_panel_open(PanelKind::Library),
        "switched, not closed"
    );
    let _toggled = app.toggle_panel(PanelKind::FootprintLibrary);
    assert!(
        !app.is_panel_open(PanelKind::Library),
        "a second toggle closes the panel in front"
    );
}
