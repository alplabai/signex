//! #858 -- a save whose file write succeeded stays a save when its git commit
//! fails. The app shows a Messages warning once per cause, keeps an
//! "uncommitted changes" state on the library, and "Commit now" retries and
//! clears it. Real dispatcher, real `LocalGitAdapter` with git on.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use signex_app::app::{Message, Signex};
use signex_app::library::messages::LibraryMessage;
use signex_app::library::state::LibraryBrowserState;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use uuid::Uuid;

fn versioned_library(dir: &Path) -> PathBuf {
    signex_library::git_identity::test_identity::install();
    let snxlib = dir.join("lib").join("lib.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    drop(
        LocalGitAdapter::init(
            &snxlib,
            manifest,
            LibraryInitOptions {
                enable_git: true,
                use_lfs: false,
            },
        )
        .expect("init library"),
    );
    snxlib
}

/// A branch-ref lock a crashed git left behind: every commit fails at once.
fn fail_commits(root: &Path) -> PathBuf {
    let head = std::fs::read_to_string(root.join(".git").join("HEAD")).unwrap();
    let branch_ref = head.trim().strip_prefix("ref: ").unwrap().to_string();
    let lock = root.join(".git").join(format!("{branch_ref}.lock"));
    let file = std::fs::File::create(&lock).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(60))
        .unwrap();
    lock
}

fn add_table(app: &mut Signex, snxlib: &Path, name: &str) {
    let path = snxlib.to_path_buf();
    for msg in [
        LibraryMessage::BrowserBeginAddTable {
            library_path: path.clone(),
        },
        LibraryMessage::BrowserSetNewTableName {
            library_path: path.clone(),
            value: name.into(),
        },
        LibraryMessage::BrowserConfirmAddTable {
            library_path: path.clone(),
        },
    ] {
        let _ = app.update(Message::Library(msg));
    }
    app.land_pending_refresh(snxlib);
}

/// Messages entries naming the library (by its unique temp directory) that
/// start with `prefix`.
fn messages(marker: &str, prefix: &str) -> usize {
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.starts_with(prefix) && e.message.contains(marker))
        .count()
}

fn head_tables(snxlib: &Path) -> Vec<String> {
    let repo = git2::Repository::open(snxlib.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let blob = repo
        .find_blob(tree.get_name("lib.snxlib").unwrap().id())
        .unwrap();
    let lf = signex_library::library_file::LibraryFile::parse(
        std::str::from_utf8(blob.content()).unwrap(),
    )
    .unwrap();
    lf.tables.keys().cloned().collect()
}

#[test]
fn a_failed_commit_warns_once_marks_the_library_and_commit_now_clears_it() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::Builder::new()
        .prefix("signex-858-")
        .tempdir()
        .expect("tempdir");
    let marker = tmp
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let snxlib = versioned_library(tmp.path());
    let root = snxlib.parent().unwrap().to_path_buf();
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    app.library
        .library_browsers
        .insert(snxlib.clone(), LibraryBrowserState::new(snxlib.clone()));
    assert!(app.library.uncommitted_in(&snxlib).is_none());

    // Two saves, both with a failing commit.
    let lock = fail_commits(&root);
    add_table(&mut app, &snxlib, "caps");
    add_table(&mut app, &snxlib, "res");

    // Both are saves: the tables are on disk and in the browser.
    let on_disk = LocalGitAdapter::open(&snxlib)
        .unwrap()
        .list_tables()
        .unwrap();
    assert_eq!(on_disk, vec!["caps".to_string(), "res".to_string()]);
    assert_eq!(
        app.library.library_browsers[&snxlib]
            .active_table
            .as_deref(),
        Some("res")
    );
    // One Messages warning for the one cause, not one per save.
    assert_eq!(messages(&marker, "Saved, not committed"), 1);
    // The library carries the persistent uncommitted state.
    let pending = app
        .library
        .uncommitted_in(&snxlib)
        .expect("uncommitted state");
    assert_eq!(pending.paths, vec!["lib.snxlib".to_string()]);
    assert!(pending.reason.contains("older than"), "{}", pending.reason);

    // "Commit now" while git still fails: reported, still uncommitted.
    let task = app.update(Message::Library(LibraryMessage::BrowserCommitPending {
        library_path: snxlib.clone(),
    }));
    assert!(app.library.is_committing(&snxlib));
    // The commit runs as the Task `update` handed back, off the update thread.
    crate::drive::drive(&mut app, task);
    assert!(!app.library.is_committing(&snxlib));
    assert!(app.library.uncommitted_in(&snxlib).is_some());
    assert_eq!(messages(&marker, "Commit now failed"), 1);
    assert_eq!(messages(&marker, "Saved, not committed"), 1, "still once");

    // Git repaired: "Commit now" commits both saves and clears the state.
    std::fs::remove_file(lock).unwrap();
    let task = app.update(Message::Library(LibraryMessage::BrowserCommitPending {
        library_path: snxlib.clone(),
    }));
    crate::drive::drive(&mut app, task);
    assert!(app.library.uncommitted_in(&snxlib).is_none());
    assert_eq!(
        head_tables(&snxlib),
        vec!["caps".to_string(), "res".to_string()]
    );
}

#[test]
fn the_next_save_commits_the_pending_changes_and_clears_the_state() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::Builder::new()
        .prefix("signex-858-")
        .tempdir()
        .expect("tempdir");
    let snxlib = versioned_library(tmp.path());
    let root = snxlib.parent().unwrap().to_path_buf();
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    app.library
        .library_browsers
        .insert(snxlib.clone(), LibraryBrowserState::new(snxlib.clone()));

    let lock = fail_commits(&root);
    add_table(&mut app, &snxlib, "caps");
    assert!(app.library.uncommitted_in(&snxlib).is_some());

    std::fs::remove_file(lock).unwrap();
    add_table(&mut app, &snxlib, "res");
    assert!(app.library.uncommitted_in(&snxlib).is_none());
    assert_eq!(
        head_tables(&snxlib),
        vec!["caps".to_string(), "res".to_string()]
    );
}

/// A pending file that was deleted since: "Commit now" commits the removal and
/// the banner clears (it used to fail for good with no file to anchor on).
#[test]
fn commit_now_succeeds_when_every_pending_file_was_deleted() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::Builder::new()
        .prefix("signex-858-")
        .tempdir()
        .expect("tempdir");
    let snxlib = versioned_library(tmp.path());
    let root = snxlib.parent().unwrap().to_path_buf();
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    let id = app.library.library_at(&snxlib).unwrap().library_id;
    let adapter = app.library.set.get_shared(id).unwrap();

    let note = root.join("note.txt");
    std::fs::write(&note, "v1").unwrap();
    adapter.commit_external_change(&note, "note").unwrap();
    std::fs::write(&note, "v2").unwrap();
    let lock = fail_commits(&root);
    adapter.commit_external_change(&note, "edit").unwrap();
    std::fs::remove_file(lock).unwrap();
    app.library.sync_uncommitted();
    assert!(app.library.uncommitted_in(&snxlib).is_some());

    std::fs::remove_file(&note).unwrap();
    let task = app.update(Message::Library(LibraryMessage::BrowserCommitPending {
        library_path: snxlib.clone(),
    }));
    crate::drive::drive(&mut app, task);
    assert!(app.library.uncommitted_in(&snxlib).is_none());
    let repo = git2::Repository::open(&root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    assert!(tree.get_name("note.txt").is_none(), "the removal landed");
}
