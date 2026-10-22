//! #152 — Components Panel library actions must not silently do nothing.
//!
//! "Add Library" on the Project section had a TODO handler that logged
//! and returned, and a failed mount from the Installed / Global headers
//! was only logged. Each now reaches the user through the shared error
//! card. Driven through the real `Message::Library` path with a real
//! temp directory.

use signex_app::app::{Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::state::ComponentsMountSource;

#[test]
fn project_add_library_says_it_is_not_available() {
    let (mut app, _t) = Signex::new();

    let _ = app.update(Message::Library(LibraryMessage::ComponentsPanelAddLibrary(
        ComponentsMountSource::Project,
    )));

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("the Project add-library request must be surfaced, not dropped");
    assert!(
        notice.detail.contains("not yet available"),
        "got {:?}",
        notice.detail
    );
}

#[test]
fn project_add_library_at_a_path_is_not_silently_dropped() {
    let (mut app, _t) = Signex::new();
    let tmp = tempfile::tempdir().expect("tempdir");

    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelAddLibraryAt {
            source: ComponentsMountSource::Project,
            path: Some(tmp.path().join("x.snxlib")),
        },
    ));

    assert!(app.document_state.error_notice.is_some());
    assert!(app.library.installed_libraries.is_empty());
}

#[test]
fn failed_installed_add_library_reaches_the_user() {
    let (mut app, _t) = Signex::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("missing.snxlib");

    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelAddLibraryAt {
            source: ComponentsMountSource::Installed,
            path: Some(missing),
        },
    ));

    assert!(
        app.library.installed_libraries.is_empty(),
        "a library that failed to mount must not be listed as installed"
    );
    let shown = app.document_state.error_notice.is_some() || app.library.recovery.is_some();
    assert!(shown, "the mount failure must be surfaced to the user");
}
