//! #814: the open-failure recovery flows must never run against a
//! read-only (stock) library.

use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::recovery::{GitMissingChoice, RecoveryDialog};

/// A real, valid stock `.snxlib` with an empty `.git/` beside it: the
/// file parses, so `LocalGitAdapter::open` fails only at the git step
/// with `Backend("git open: ...")` - the exact shape that opens the
/// *Git missing* dialog for an ordinary library.
fn broken_stock() -> (tempfile::TempDir, std::path::PathBuf, Signex) {
    use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
    use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
    use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

    let dir = tempfile::tempdir().unwrap();
    let lib_file = dir.path().join("Stock.snxlib");
    LocalGitAdapter::init(
        &lib_file,
        SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: uuid::Uuid::now_v7(),
            library: LibrarySection {
                name: "Stock".into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        },
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    match LocalGitAdapter::open(&lib_file) {
        Err(signex_library::LibraryError::Backend(m)) if m.starts_with("git open") => {}
        Err(other) => panic!("fixture must fail with a git-open error, got {other:?}"),
        Ok(_) => panic!("fixture must fail to open"),
    }
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
    let before = std::fs::read(&lib_file).unwrap();
    app.library.recovery = Some(RecoveryDialog::GitMissing {
        path: lib_file.clone(),
        remote: None,
        error: None,
    });
    let _ = app.update(Message::Library(LibraryMessage::RecoveryGitMissing(
        GitMissingChoice::ReInit,
    )));
    assert!(app.library.recovery.is_none());
    // A re-init would populate `.git/` (HEAD, config, ...); the empty
    // directory must stay empty, whatever the git identity is.
    let entries = std::fs::read_dir(dir.path().join(".git")).unwrap().count();
    assert_eq!(entries, 0, "re-init wrote into the read-only library");
    assert_eq!(std::fs::read(&lib_file).unwrap(), before);
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(notice.detail.contains("read-only"), "{}", notice.detail);
}
