//! #1729 — library repos carry the managed `.gitignore` block, so OS and
//! temp clutter never reaches the commit ledger, and one unreadable pending
//! file never stalls the commits after it.

use super::*;
use crate::library_file::{FORMAT_TOKEN, LibrarySection};
use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_types::project_vcs_files::GITIGNORE_BLOCK_BODY;

fn manifest(name: &str) -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: uuid::Uuid::now_v7(),
        library: LibrarySection {
            name: name.into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    }
}

fn versioned(dir: &tempfile::TempDir) -> LocalGitAdapter {
    crate::git_identity::test_identity::install();
    LocalGitAdapter::init(
        dir.path().join("Lib").join("Lib.snxlib"),
        manifest("Lib"),
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init")
}

fn class(key: &str) -> crate::library_file::ClassEntry {
    crate::library_file::ClassEntry {
        key: key.into(),
        label: key.into(),
    }
}

fn head_blob(root: &Path, name: &str) -> Option<Vec<u8>> {
    let repo = git2::Repository::open(root).unwrap();
    let tree = repo.head().ok()?.peel_to_commit().unwrap().tree().unwrap();
    let entry = tree.get_path(Path::new(name)).ok()?;
    Some(repo.find_blob(entry.id()).unwrap().content().to_vec())
}

fn carries_the_block(text: &str) -> bool {
    GITIGNORE_BLOCK_BODY
        .lines()
        .filter(|line| !line.starts_with('#'))
        .all(|rule| text.lines().any(|line| line == rule))
}

#[test]
fn a_new_versioned_library_commits_the_managed_ignore_block() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = versioned(&dir);
    let text = fs::read_to_string(adapter.root().join(".gitignore")).expect(".gitignore");
    assert!(carries_the_block(&text), "{text}");
    let committed = head_blob(adapter.root(), ".gitignore").expect("in the first commit");
    assert_eq!(committed, text.as_bytes());
}

#[test]
fn an_unversioned_library_gets_no_ignore_file() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = LocalGitAdapter::init(
        dir.path().join("Plain").join("Plain.snxlib"),
        manifest("Plain"),
        LibraryInitOptions::default(),
    )
    .unwrap();
    assert!(!adapter.root().join(".gitignore").exists());
}

/// A library made before the block existed. Opening it writes nothing
/// (an open may be read-only); its next commit merges the block into the
/// user's file, keeps their lines and records it.
#[test]
fn an_older_library_gets_the_block_with_its_next_commit() {
    let dir = tempfile::tempdir().unwrap();
    let root = older_library(&dir, "my-rule\n");
    let ignore = root.join(".gitignore");

    let adapter = LocalGitAdapter::open(root.join("Lib.snxlib")).unwrap();
    assert_eq!(fs::read_to_string(&ignore).unwrap(), "my-rule\n");
    assert_eq!(adapter.pending_changes(), None);

    adapter.add_library_class(class("res"), "add res").unwrap();

    let text = fs::read_to_string(&ignore).unwrap();
    assert!(text.lines().any(|line| line == "my-rule"), "{text}");
    assert!(carries_the_block(&text), "{text}");
    assert_eq!(adapter.pending_changes(), None);
    assert_eq!(head_blob(&root, ".gitignore").unwrap(), text.as_bytes());
}

/// A versioned library whose committed `.gitignore` is `user_lines` only,
/// committed by plain git as a library from before #1729 would have it.
fn older_library(dir: &tempfile::TempDir, user_lines: &str) -> PathBuf {
    let root = versioned(dir).root().to_path_buf();
    fs::write(root.join(".gitignore"), user_lines).unwrap();
    let repo = git2::Repository::open(&root).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(".gitignore")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    let sig = git2::Signature::now("User", "user@example.test").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "user ignore", &tree, &[&parent])
        .unwrap();
    root
}

#[test]
fn os_and_temp_clutter_never_reaches_the_ledger_or_a_commit() {
    let dir = tempfile::tempdir().unwrap();
    // An older library: the rules hold before its `.gitignore` has them.
    let root = older_library(&dir, "");
    let clutter = [".DS_Store", "symbols/.DS_Store", "symbols/x.tmp", "x.tmp"];
    fs::create_dir_all(root.join("symbols")).unwrap();
    for file in clutter {
        fs::write(root.join(file), "clutter").unwrap();
    }

    let adapter = LocalGitAdapter::open(root.join("Lib.snxlib")).unwrap();
    assert_eq!(adapter.pending_changes(), None, "nothing to commit");
    adapter.add_library_class(class("res"), "add res").unwrap();

    for file in clutter {
        assert_eq!(head_blob(&root, file), None, "{file} was committed");
    }
}

/// Unix only: an unreadable file is made with `chmod 000`, which Windows
/// has no equivalent of.
#[cfg(unix)]
#[test]
fn one_unreadable_pending_file_does_not_stall_the_next_commit() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = versioned(&dir).root().to_path_buf();
    fs::create_dir_all(root.join("symbols")).unwrap();
    let bad = root.join("symbols").join("bad.snxsym");
    fs::write(&bad, "unreadable").unwrap();
    fs::set_permissions(&bad, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&bad).is_ok() {
        // Running as root: permissions are not enforced.
        return;
    }
    let adapter = LocalGitAdapter::open(root.join("Lib.snxlib")).unwrap();
    assert!(adapter.pending_changes().is_some(), "the file is pending");

    for key in ["res", "cap"] {
        adapter.add_library_class(class(key), "add class").unwrap();
        let on_disk = fs::read(root.join("Lib.snxlib")).unwrap();
        assert_eq!(
            head_blob(&root, "Lib.snxlib").unwrap(),
            on_disk,
            "the save's own change is committed"
        );
        let pending = adapter
            .pending_changes()
            .expect("the bad file stays pending");
        assert_eq!(pending.paths, vec!["symbols/bad.snxsym".to_string()]);
        let held = adapter.take_held_back_files();
        assert_eq!(held.len(), 1, "reported on every commit it blocks");
        assert_eq!(held[0].path, "symbols/bad.snxsym");
        assert!(!held[0].reason.is_empty());
    }
    fs::set_permissions(&bad, fs::Permissions::from_mode(0o644)).unwrap();
}

/// A branch-ref lock a crashed git left behind: every commit fails.
fn fail_commits(root: &Path) -> PathBuf {
    let head = fs::read_to_string(root.join(".git").join("HEAD")).unwrap();
    let branch = head.trim().strip_prefix("ref: ").unwrap().to_owned();
    let lock = root.join(".git").join(format!("{branch}.lock"));
    let file = fs::File::create(&lock).unwrap();
    file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(60))
        .unwrap();
    lock
}

/// Put `rel` into the ledger the way a save whose commit failed does.
fn pending_from_a_failed_commit(adapter: &LocalGitAdapter, rel: &str) {
    let lock = fail_commits(adapter.root());
    let outcome = adapter
        .commit_external_change(&adapter.root().join(rel), "save")
        .unwrap();
    assert!(!outcome.is_recorded(), "the commit failed");
    fs::remove_file(lock).unwrap();
}

/// Commit `rel` as it is on disk with plain git.
fn plain_commit(root: &Path, rel: &str) {
    let repo = git2::Repository::open(root).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(rel)).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    let sig = git2::Signature::now("User", "user@example.test").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "plain", &tree, &[&parent])
        .unwrap();
}

/// Review #1729: only the managed block's rules drop a ledger file. A file
/// the user's own rule ignores was committed on purpose by the caller, and
/// the sweep records it like any other pending file.
#[test]
fn a_pending_file_under_a_user_ignore_rule_is_still_committed() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = versioned(&dir);
    let root = adapter.root().to_path_buf();
    let ignore = root.join(".gitignore");
    let user = format!("{}*.pdf\n", fs::read_to_string(&ignore).unwrap());
    fs::write(&ignore, user).unwrap();
    plain_commit(&root, ".gitignore");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/sheet.pdf"), "pdf").unwrap();
    pending_from_a_failed_commit(&adapter, "docs/sheet.pdf");

    adapter.add_library_class(class("res"), "add res").unwrap();

    assert_eq!(head_blob(&root, "docs/sheet.pdf").unwrap(), b"pdf");
    assert_eq!(adapter.pending_changes(), None);
}

#[test]
fn a_tracked_file_under_a_user_ignore_rule_is_still_committed() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = versioned(&dir);
    let root = adapter.root().to_path_buf();
    fs::write(root.join("keep.log"), "v1").unwrap();
    plain_commit(&root, "keep.log");
    let ignore = root.join(".gitignore");
    let user = format!("{}*.log\n", fs::read_to_string(&ignore).unwrap());
    fs::write(&ignore, user).unwrap();
    plain_commit(&root, ".gitignore");
    fs::write(root.join("keep.log"), "v2").unwrap();
    pending_from_a_failed_commit(&adapter, "keep.log");

    adapter.add_library_class(class("res"), "add res").unwrap();

    assert_eq!(head_blob(&root, "keep.log").unwrap(), b"v2");
    assert_eq!(adapter.pending_changes(), None);
}

/// Review #1729: a pending path the commit cannot hash (here a directory,
/// which opens fine as a file handle) is the only one held back; the good
/// pending file next to it is committed.
#[test]
fn only_the_pending_path_that_cannot_be_hashed_is_held_back() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = versioned(&dir);
    let root = adapter.root().to_path_buf();
    fs::create_dir_all(root.join("symbols/sub")).unwrap();
    fs::write(root.join("symbols/sub/x.snxsym"), "x").unwrap();
    fs::write(root.join("symbols/good.snxsym"), "good").unwrap();
    pending_from_a_failed_commit(&adapter, "symbols/sub");
    pending_from_a_failed_commit(&adapter, "symbols/good.snxsym");

    adapter.add_library_class(class("res"), "add res").unwrap();

    assert_eq!(head_blob(&root, "symbols/good.snxsym").unwrap(), b"good");
    let held = adapter.take_held_back_files();
    assert_eq!(
        held.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
        vec!["symbols/sub"]
    );
    let pending = adapter
        .pending_changes()
        .expect("the directory stays pending");
    assert_eq!(pending.paths, vec!["symbols/sub".to_string()]);
}

/// Review #1729: when the merged ignore block rides in a commit, the
/// commit message says so.
#[test]
fn a_commit_that_adds_the_ignore_block_names_it_in_its_message() {
    let dir = tempfile::tempdir().unwrap();
    let root = older_library(&dir, "");
    let adapter = LocalGitAdapter::open(root.join("Lib.snxlib")).unwrap();

    adapter.add_library_class(class("res"), "add res").unwrap();

    let repo = git2::Repository::open(&root).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let message = head.message().unwrap();
    assert!(message.contains("also commits: .gitignore"), "{message}");
}

/// #1734 creates pending libraries off the update loop and finishes a
/// left-behind one with `complete_init`; its first commit carries the
/// managed block too (both go through `commit_initial`).
#[test]
fn a_completed_init_commits_the_managed_ignore_block() {
    crate::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Left").join("Left.snxlib");
    drop(LocalGitAdapter::init(&file, manifest("Left"), LibraryInitOptions::default()).unwrap());
    let root = file.parent().unwrap().to_path_buf();
    fs::write(root.join(".gitignore"), "mine\n").unwrap();

    LocalGitAdapter::complete_init(
        &file,
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .unwrap();

    let text = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(carries_the_block(&text), "{text}");
    assert!(text.lines().any(|line| line == "mine"), "{text}");
    assert_eq!(head_blob(&root, ".gitignore").unwrap(), text.as_bytes());
}
