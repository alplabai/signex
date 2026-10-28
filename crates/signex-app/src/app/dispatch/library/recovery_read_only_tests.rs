//! #814: the open-failure recovery flows must never run against a
//! read-only (stock) library.

use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::recovery::{GitMissingChoice, RecoveryDialog};

fn broken_stock() -> (tempfile::TempDir, std::path::PathBuf, Signex) {
    let dir = tempfile::tempdir().unwrap();
    let lib_file = dir.path().join("Stock.snxlib");
    std::fs::write(&lib_file, "not a library").unwrap();
    let (mut app, _boot) = Signex::new();
    app.library.stock_library_root = Some(lib_file.clone());
    (dir, lib_file, app)
}

#[test]
fn a_broken_stock_library_raises_the_error_card_not_a_recovery_dialog() {
    let (_dir, lib_file, mut app) = broken_stock();
    let _ = app.update(Message::Library(LibraryMessage::OpenLibraryAt(Some(
        lib_file,
    ))));
    assert!(app.library.recovery.is_none());
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(notice.detail.contains("read-only"), "{}", notice.detail);
}

#[test]
fn git_reinit_never_writes_to_a_read_only_library() {
    let (dir, lib_file, mut app) = broken_stock();
    app.library.recovery = Some(RecoveryDialog::GitMissing {
        path: lib_file.clone(),
        remote: None,
        error: None,
    });
    let _ = app.update(Message::Library(LibraryMessage::RecoveryGitMissing(
        GitMissingChoice::ReInit,
    )));
    assert!(app.library.recovery.is_none());
    assert!(!dir.path().join(".git").exists());
    assert_eq!(std::fs::read(&lib_file).unwrap(), b"not a library");
}
