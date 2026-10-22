//! #152 — Components Panel library actions must not silently do nothing.
//!
//! "Add Library" on the Project section had a TODO handler that logged
//! and returned, and a failed mount from the Installed / Global headers
//! was only logged. Each now reaches the user through the shared error
//! card. Driven through the real `Message::Library` path with a real
//! temp directory.

use signex_app::app::{Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::recovery::RecoveryDialog;
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

/// A `.snxlib` that exists but cannot be parsed: a mount failure that
/// opens no recovery dialog, so only the error card can reach the user.
fn unparseable_library(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("broken.snxlib");
    std::fs::write(&path, "this is not a library manifest").expect("write broken library");
    path
}

#[test]
fn non_recoverable_mount_failure_raises_the_error_card() {
    let (mut app, _t) = Signex::new();
    let tmp = tempfile::tempdir().expect("tempdir");

    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelAddLibraryAt {
            source: ComponentsMountSource::Installed,
            path: Some(unparseable_library(tmp.path())),
        },
    ));

    assert!(
        app.library.recovery.is_none(),
        "this failure is not recoverable, so no dialog may open"
    );
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("a non-recoverable mount failure must raise the error card");
    assert!(
        notice.detail.contains("Cannot add library"),
        "{:?}",
        notice.detail
    );
    assert!(app.library.installed_libraries.is_empty());
}

#[test]
fn mount_failure_is_surfaced_even_while_a_recovery_dialog_is_open() {
    let (mut app, _t) = Signex::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let earlier = tmp.path().join("earlier.snxlib");
    app.library.recovery = Some(RecoveryDialog::LibraryMissing {
        path: earlier.clone(),
    });

    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelAddLibraryAt {
            source: ComponentsMountSource::Installed,
            path: Some(unparseable_library(tmp.path())),
        },
    ));

    assert!(
        matches!(
            app.library.recovery,
            Some(RecoveryDialog::LibraryMissing { ref path }) if *path == earlier
        ),
        "the earlier recovery dialog must be left alone"
    );
    assert!(
        app.document_state.error_notice.is_some(),
        "the new failure produced no dialog of its own, so the card must show"
    );
}

/// Makes `global_libraries.toml` unwritable by putting a directory where
/// the file belongs; removed on drop so a failing assertion cannot leave
/// the shared per-process config root poisoned for the other tests.
struct UnwritableGlobalList {
    path: std::path::PathBuf,
}

impl UnwritableGlobalList {
    fn new() -> Self {
        let path = signex_app::panels::components_panel::global_prefs::prefs_path()
            .expect("the test redirect always resolves a config root");
        std::fs::create_dir_all(&path).expect("make global_libraries.toml a directory");
        Self { path }
    }
}

impl Drop for UnwritableGlobalList {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn failed_promote_keeps_the_library_listed_and_shows_the_error() {
    let (mut app, _t) = Signex::new();
    let lib = std::path::PathBuf::from("promoted.snxlib");
    app.library.installed_libraries.push(lib.clone());
    let _unwritable = UnwritableGlobalList::new();

    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelPromoteToGlobal(lib.clone()),
    ));

    assert!(
        app.library.installed_libraries.contains(&lib),
        "a failed promote must not drop the library from the panel"
    );
    assert!(app.library.global_libraries.iter().all(|e| e.path != lib));
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("the failed global_libraries.toml write must be reported");
    assert!(
        notice.detail.contains("promote to global"),
        "{:?}",
        notice.detail
    );
}

#[test]
fn open_library_failure_raises_the_error_card() {
    let (mut app, _t) = Signex::new();
    let tmp = tempfile::tempdir().expect("tempdir");

    let _ = app.update(Message::Library(LibraryMessage::OpenLibraryAt(Some(
        unparseable_library(tmp.path()),
    ))));

    assert!(app.library.recovery.is_none());
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("File > Library > Open Library must not only log a failure");
    assert!(
        notice.detail.contains("Cannot open library"),
        "{:?}",
        notice.detail
    );
}
