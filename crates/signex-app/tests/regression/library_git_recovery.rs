//! The *Git missing* recovery dialog (#141): a refused re-init must leave the
//! dialog open with the reason, and only a successful re-init may close it.
//!
//! The refusal here is a library file that does not parse, not a missing git
//! identity: forcing "no identity" needs process-global libgit2 / env changes
//! that would race the other tests in this binary. The no-identity refusal
//! itself is covered in `signex-library/tests/git_identity.rs`
//! (`recover_init` returns the same `Err` this dialog displays). The success
//! half uses a repo-local identity, so it passes with or without a global one.

use signex_app::app::Signex;
use signex_app::app::contracts::Message;
use signex_app::library::messages::LibraryMessage;
use signex_app::library::recovery::{GitMissingChoice, RecoveryDialog};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

fn valid_snxlib_bytes(scratch: &std::path::Path) -> Vec<u8> {
    let path = scratch.join("seed").join("seed.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: uuid::Uuid::now_v7(),
        library: LibrarySection {
            name: "seed".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(&path, manifest, LibraryInitOptions::default()).unwrap();
    std::fs::read(&path).unwrap()
}

fn re_init(app: &mut Signex) {
    let _ = app.update(Message::Library(LibraryMessage::RecoveryGitMissing(
        GitMissingChoice::ReInit,
    )));
}

#[test]
fn a_refused_reinit_keeps_the_dialog_open_and_a_successful_one_closes_it() {
    let scratch = tempfile::tempdir().unwrap();
    let good = valid_snxlib_bytes(scratch.path());

    let dir = tempfile::tempdir().unwrap();
    let lib_file = dir.path().join("lib.snxlib");
    std::fs::write(&lib_file, "not a library").unwrap();

    let (mut app, _boot) = Signex::new();
    app.library.recovery = Some(RecoveryDialog::GitMissing {
        path: lib_file.clone(),
        remote: None,
        error: None,
    });

    // Refused: the dialog stays open and carries the reason.
    re_init(&mut app);
    match &app.library.recovery {
        Some(RecoveryDialog::GitMissing { error: Some(e), .. }) => {
            assert!(!e.is_empty(), "the reason must be shown");
        }
        other => panic!("dialog must stay open with the error, got {other:?}"),
    }

    // Same click, now able to succeed (valid library, repo-local identity).
    std::fs::write(&lib_file, &good).unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "Test User").unwrap();
    cfg.set_str("user.email", "test@example.org").unwrap();
    re_init(&mut app);
    assert!(
        app.library.recovery.is_none(),
        "dialog closes on a successful re-init, got {:?}",
        app.library.recovery
    );
}
