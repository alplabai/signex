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
//! (Caner, 2026-11-20; mockup row 138); the dialog's close leaves it
//! unopened.

use std::path::{Path, PathBuf};

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
            ..Default::default()
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

/// A library in `dir` whose `.git/` is empty, so its history is missing,
/// opened in a fresh app: the open raises Git Repository Missing.
fn open_without_history(dir: &Path) -> (Signex, PathBuf) {
    use crate::library_read_only_mount::library_with_rows;

    let (snxlib, _rows) = library_with_rows(dir, "Passives", 1);
    std::fs::create_dir(dir.join(".git")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &snxlib);
    assert!(app.library.library_at(&snxlib).is_none());
    (app, snxlib)
}

/// Open the library at `snxlib`; its missing history raises the dialog.
fn open(app: &mut Signex, snxlib: &Path) {
    let task = app.update(Message::Library(LibraryMessage::OpenLibraryAt(Some(
        snxlib.to_path_buf(),
    ))));
    crate::drive::drive(app, task);
    assert!(
        matches!(
            app.library.recovery,
            Some(RecoveryDialog::GitMissing { .. })
        ),
        "the open raises Git Repository Missing, got {:?}",
        app.library.recovery
    );
}

fn choose(app: &mut Signex, choice: GitMissingChoice) {
    let task = app.update(Message::Library(LibraryMessage::RecoveryGitMissing(choice)));
    crate::drive::drive(app, task);
}

fn strip(s: String) -> String {
    s.replace(['\u{2068}', '\u{2069}'], "")
}

/// Skip for now on Git Repository Missing mounts the library read-only: it
/// is open, an edit is refused with the reason from the catalog, and no file
/// of the library changes.
#[test]
fn skip_for_now_opens_the_library_read_only_and_refuses_an_edit() {
    use crate::library_read_only_mount::{TABLE, bytes_of};
    use signex_i18n::RenderTranslatable;

    let tmp = tempfile::tempdir().unwrap();
    let (mut app, snxlib) = open_without_history(tmp.path());
    let before = bytes_of(tmp.path());

    choose(&mut app, GitMissingChoice::Skip);
    assert!(app.library.recovery.is_none(), "dialog closes");
    assert!(
        app.library.library_at(&snxlib).is_some(),
        "the library is open"
    );
    assert!(
        app.library.is_read_only(&snxlib),
        "the library is read-only"
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
        .expect("the refused edit must say why");
    assert_eq!(strip(notice.title.render_en_us()), "Library Is Read-Only");
    let detail = strip(notice.detail.render_en_us());
    assert!(
        detail.contains("\"Passives\" is open read-only because its Git history is missing"),
        "{detail}"
    );

    assert_eq!(
        before,
        bytes_of(tmp.path()),
        "nothing of the library may be written"
    );
    let git_entries = std::fs::read_dir(tmp.path().join(".git")).unwrap().count();
    assert_eq!(git_entries, 0, "nothing written into .git/");
}

/// The dialog's close (x, Esc) is not Skip for now: the library stays
/// unopened and nothing is written.
#[test]
fn the_close_leaves_the_library_unopened() {
    use crate::library_read_only_mount::bytes_of;

    let tmp = tempfile::tempdir().unwrap();
    let (mut app, snxlib) = open_without_history(tmp.path());
    let before = bytes_of(tmp.path());

    choose(&mut app, GitMissingChoice::Cancel);
    assert!(app.library.recovery.is_none(), "dialog closes");
    assert!(app.library.library_at(&snxlib).is_none(), "not opened");
    assert!(!app.library.is_history_missing(&snxlib));
    assert_eq!(before, bytes_of(tmp.path()));
}

/// A library open read-only for want of its history, closed and opened
/// again, raises Git Repository Missing again, where Re-init is offered.
#[test]
fn closing_and_opening_a_skipped_library_raises_the_dialog_again() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, snxlib) = open_without_history(tmp.path());
    choose(&mut app, GitMissingChoice::Skip);
    assert!(app.library.is_history_missing(&snxlib));

    let task = app.update(Message::Library(LibraryMessage::CloseLibrary(
        snxlib.clone(),
    )));
    crate::drive::drive(&mut app, task);
    assert!(app.library.library_at(&snxlib).is_none(), "closed");
    assert!(!app.library.is_history_missing(&snxlib));

    open(&mut app, &snxlib);
    assert!(app.library.library_at(&snxlib).is_none(), "not opened yet");
}

/// The footprint editor writes its file itself, past the adapter: a save of
/// a footprint inside a library open read-only for want of its history is
/// refused with the history reason, and the file is untouched.
#[test]
fn a_primitive_save_in_a_skipped_library_writes_nothing_and_says_why() {
    use crate::library_read_only_mount::{bytes_of, write_footprint};
    use signex_app::app::FileMsg;
    use signex_app::diagnostics;
    use signex_app::library::messages::{FootprintEditorMsg, PrimitiveEdit};

    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let fp_path = tmp.path().join("footprints").join("skipped_fp.snxfpt");
    let (mut app, snxlib) = open_without_history(tmp.path());
    let original = write_footprint(&fp_path);
    choose(&mut app, GitMissingChoice::Skip);
    assert!(app.library.is_history_missing(&snxlib));
    let before = bytes_of(tmp.path());

    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(fp_path.clone()))),
    );
    assert!(
        app.document_state.footprint_editors.contains_key(&fp_path),
        "the footprint opens in an editor tab"
    );
    let task = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: fp_path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetName("renamed".into())),
    }));
    crate::drive::drive(&mut app, task);
    crate::settle_support::settle(
        &mut app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: fp_path.clone(),
            msg: PrimitiveEdit::Save,
        }),
    );

    assert_eq!(original, std::fs::read(&fp_path).unwrap(), "file untouched");
    assert_eq!(before, bytes_of(tmp.path()));
    assert!(
        diagnostics::recent_entries().iter().any(|entry| {
            strip(entry.message.clone())
                .contains("\"Passives\" is open read-only because its Git history is missing")
        }),
        "the refused save says why"
    );
}
