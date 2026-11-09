//! Components panel layout (maintainer decision of 2026-11-08): one flat
//! library list with source chips and a header "+ Add" menu. Driven through
//! the real `Message::Library` path.

use signex_app::app::{Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::state::ComponentsMountSource;
use std::path::PathBuf;

fn send(app: &mut Signex, message: LibraryMessage) {
    let _ = app.update(Message::Library(message));
}

#[test]
fn a_library_row_expands_and_collapses_on_its_toggle() {
    let (mut app, _t) = Signex::new();
    let root = PathBuf::from("/tmp/lib.snxlib");
    let collapsed = |app: &Signex| {
        app.library
            .components_panel
            .collapsed_libraries
            .contains(&root)
    };

    assert!(!collapsed(&app), "libraries start expanded");
    send(
        &mut app,
        LibraryMessage::ComponentsPanelToggleLibrary(root.clone()),
    );
    assert!(collapsed(&app));
    send(
        &mut app,
        LibraryMessage::ComponentsPanelToggleLibrary(root.clone()),
    );
    assert!(!collapsed(&app));
}

#[test]
fn the_source_chip_is_panel_state_and_all_is_the_default() {
    let (mut app, _t) = Signex::new();
    assert_eq!(app.library.components_panel.source_filter, None);

    send(
        &mut app,
        LibraryMessage::ComponentsPanelSourceFilterChanged(Some(ComponentsMountSource::Global)),
    );
    assert_eq!(
        app.library.components_panel.source_filter,
        Some(ComponentsMountSource::Global)
    );
    send(
        &mut app,
        LibraryMessage::ComponentsPanelSourceFilterChanged(None),
    );
    assert_eq!(app.library.components_panel.source_filter, None);
}

#[test]
fn the_add_menu_opens_and_closes_when_an_item_is_picked() {
    let (mut app, _t) = Signex::new();
    send(&mut app, LibraryMessage::ComponentsPanelAddMenuToggled);
    assert!(app.library.components_panel.add_menu_open);

    // Project routes to the existing handler, which refuses with a notice.
    send(
        &mut app,
        LibraryMessage::ComponentsPanelAddLibrary(ComponentsMountSource::Project),
    );
    assert!(!app.library.components_panel.add_menu_open);
    assert!(app.document_state.error_notice.is_some());

    send(&mut app, LibraryMessage::ComponentsPanelAddMenuToggled);
    send(&mut app, LibraryMessage::ComponentsPanelAddMenuToggled);
    assert!(
        !app.library.components_panel.add_menu_open,
        "a second press closes it"
    );
}
