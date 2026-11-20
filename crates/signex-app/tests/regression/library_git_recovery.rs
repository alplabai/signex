//! The *Git missing* recovery dialog (#141): a refused re-init must leave the
//! dialog open with the reason, and only a successful re-init may close it.
//!
//! The refusal here is a library file that does not parse, not a missing git
//! identity: forcing "no identity" needs process-global libgit2 / env changes
//! that would race the other tests in this binary. The no-identity refusal
//! itself is covered in `signex-library/tests/git_identity.rs`
//! (`recover_init` returns the same `Err` this dialog displays). The success
//! half uses a repo-local identity, so it passes with or without a global one.
//!
//! Skip for now opens the library read-only until its history is restored
//! (Caner, 2026-11-20; mockup row 138).

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
    let task = app.update(Message::Library(LibraryMessage::RecoveryGitMissing(
        GitMissingChoice::ReInit,
    )));
    crate::drive::drive(app, task);
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

/// #1741: Re-init in a folder that is another repo with history, not tracking
/// the library, is refused in the dialog and commits nothing.
#[test]
fn reinit_in_another_repo_is_refused_in_the_dialog_and_commits_nothing() {
    let scratch = tempfile::tempdir().unwrap();
    let good = valid_snxlib_bytes(scratch.path());

    let dir = tempfile::tempdir().unwrap();
    let lib_file = dir.path().join("lib.snxlib");
    std::fs::write(&lib_file, &good).unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "Test User").unwrap();
    cfg.set_str("user.email", "test@example.org").unwrap();
    std::fs::write(dir.path().join("project.snxprj"), "project").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("project.snxprj"))
        .unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("Test User", "test@example.org").unwrap();
    let before = repo
        .commit(Some("HEAD"), &sig, &sig, "project", &tree, &[])
        .unwrap();

    let (mut app, _boot) = Signex::new();
    app.library.recovery = Some(RecoveryDialog::GitMissing {
        path: lib_file,
        remote: None,
        error: None,
    });
    re_init(&mut app);

    match &app.library.recovery {
        Some(RecoveryDialog::GitMissing { error: Some(e), .. }) => {
            assert!(e.contains("does not track this library"), "{e}");
        }
        other => panic!("dialog must stay open with the reason, got {other:?}"),
    }
    assert_eq!(repo.head().unwrap().target().unwrap(), before);
}

/// Skip for now, and the dialog's close (x, Esc), on Git Repository Missing
/// mount the library read-only: it is open, an edit is refused with the
/// reason from the catalog, and no file of the library changes.
#[test]
fn skip_for_now_opens_the_library_read_only_and_refuses_an_edit() {
    use crate::library_read_only_mount::{TABLE, bytes_of, library_with_rows};
    use signex_i18n::RenderTranslatable;

    for choice in [GitMissingChoice::Skip, GitMissingChoice::Cancel] {
        let tmp = tempfile::tempdir().unwrap();
        let (snxlib, _rows) = library_with_rows(tmp.path(), "Passives", 1);
        // An empty `.git/` does not open: the library's history is missing.
        std::fs::create_dir(tmp.path().join(".git")).unwrap();

        let (mut app, _boot) = Signex::new();
        let task = app.update(Message::Library(LibraryMessage::OpenLibraryAt(Some(
            snxlib.clone(),
        ))));
        crate::drive::drive(&mut app, task);
        assert!(
            matches!(
                app.library.recovery,
                Some(RecoveryDialog::GitMissing { .. })
            ),
            "{choice:?}: the open raises Git Repository Missing, got {:?}",
            app.library.recovery
        );
        assert!(app.library.library_at(&snxlib).is_none());
        let before = bytes_of(tmp.path());

        let task = app.update(Message::Library(LibraryMessage::RecoveryGitMissing(choice)));
        crate::drive::drive(&mut app, task);
        assert!(app.library.recovery.is_none(), "{choice:?}: dialog closes");
        assert!(
            app.library.library_at(&snxlib).is_some(),
            "{choice:?}: the library is open"
        );
        assert!(
            app.library.is_read_only(&snxlib),
            "{choice:?}: the library is read-only"
        );

        app.document_state.error_notice = None;
        let task = app.update(Message::Library(LibraryMessage::BrowserAddComponent {
            library_path: snxlib.clone(),
            table: Some(TABLE.to_string()),
        }));
        crate::drive::drive(&mut app, task);
        let notice = app
            .document_state
            .error_notice
            .as_ref()
            .unwrap_or_else(|| panic!("{choice:?}: the refused edit must say why"));
        let strip = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(strip(notice.title.render_en_us()), "Library Is Read-Only");
        let detail = strip(notice.detail.render_en_us());
        assert!(
            detail.contains("\"Passives\" is open read-only because its Git history is missing"),
            "{choice:?}: {detail}"
        );

        assert_eq!(
            before,
            bytes_of(tmp.path()),
            "{choice:?}: nothing of the library may be written"
        );
        let git_entries = std::fs::read_dir(tmp.path().join(".git")).unwrap().count();
        assert_eq!(git_entries, 0, "{choice:?}: nothing written into .git/");
    }
}
