//! #2337: index updates after a commit, under git's `index.lock`.

use std::fs;
use std::path::Path;

use super::helpers::{stage_and_commit, stage_and_commit_with};
use super::index_lock::{LockHook, set_index_update_hook};
use super::{LibraryInitOptions, LocalGitAdapter};

fn signature() -> git2::Signature<'static> {
    git2::Signature::now("Test", "test@example.test").unwrap()
}

fn manifest() -> crate::library_file::SnxlibManifest {
    crate::library_file::SnxlibManifest {
        format: crate::library_file::FORMAT_TOKEN.into(),
        library_id: uuid::Uuid::now_v7(),
        library: crate::library_file::LibrarySection {
            name: "lib".into(),
            description: None,
        },
        mode: crate::manifest::LibraryMode::default(),
        workflow: crate::manifest::WorkflowConfig::default(),
        users: crate::manifest::UsersConfig::default(),
        classes: Vec::new(),
    }
}

/// Hold `index.lock` in `root` as another git process would. The first
/// index update that finds it held lets it go, so the update succeeds only
/// by retrying. True once that happened.
fn hold_index_lock_until_met(root: &Path) -> std::rc::Rc<std::cell::Cell<bool>> {
    let lock = root.join(".git").join("index.lock");
    fs::write(&lock, b"").unwrap();
    let met = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = met.clone();
    set_index_update_hook(LockHook::FoundHeld, move || {
        fs::remove_file(&lock).unwrap();
        flag.set(true);
    });
    met
}

fn index_has(root: &Path, name: &str) -> bool {
    let repo = git2::Repository::open(root).unwrap();
    let index = repo.index().unwrap();
    index.get_path(Path::new(name), 0).is_some()
}

/// A library made in a repo whose `index.lock` another git process holds
/// waits for it, then stages its files.
#[test]
fn init_waits_for_an_index_lock_it_found_held() {
    crate::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    git2::Repository::init(dir.path()).unwrap();
    let met = hold_index_lock_until_met(dir.path());

    let options = LibraryInitOptions {
        enable_git: true,
        use_lfs: false,
    };
    LocalGitAdapter::init(dir.path().join("lib.snxlib"), manifest(), options).expect("init");

    assert!(met.get(), "the index update never met the held lock");
    assert!(index_has(dir.path(), "lib.snxlib"));
    assert!(!dir.path().join(".git/index.lock").exists());
}

/// Re-init while another git process holds `index.lock` waits for it,
/// then records the library's files in the index.
#[test]
fn recover_init_waits_for_an_index_lock_it_found_held() {
    crate::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib.snxlib");
    let plain = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    LocalGitAdapter::init(&lib, manifest(), plain).expect("plain library");
    LocalGitAdapter::recover_init(&lib).expect("first re-init");
    fs::create_dir_all(dir.path().join("symbols")).unwrap();
    fs::write(dir.path().join("symbols/r.snxsym"), "sym").unwrap();
    let met = hold_index_lock_until_met(dir.path());

    LocalGitAdapter::recover_init(&lib).expect("re-init");

    assert!(met.get(), "the index update never met the held lock");
    assert!(index_has(dir.path(), "symbols/r.snxsym"));
}

/// Commit the files `contents` names on top of HEAD (or as the first
/// commit) on `refname`, through the index.
fn commit_files(
    repo: &git2::Repository,
    refname: &str,
    parents: &[&git2::Commit<'_>],
    contents: &[(&str, &str)],
) -> git2::Oid {
    let root = repo.workdir().unwrap();
    let mut index = repo.index().unwrap();
    for (name, text) in contents {
        fs::write(root.join(name), text).unwrap();
        index.add_path(Path::new(name)).unwrap();
    }
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = signature();
    repo.commit(Some(refname), &sig, &sig, "commit", &tree, parents)
        .unwrap()
}

/// Another process holds `index.lock` while our save updates the index,
/// then publishes an index with a path it staged and unlocks. Our
/// update must not write back the index it saw before that publish.
#[test]
fn an_index_published_by_the_lock_holder_keeps_its_staged_path() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let sig = signature();
    fs::write(dir.path().join("file.txt"), "one\n").unwrap();
    stage_and_commit(&repo, &[], Path::new("file.txt"), None, &sig, "first")
        .unwrap()
        .expect("first commit");

    // The index the lock holder will publish: ours plus `other.txt`.
    let git = repo.path().to_path_buf();
    fs::write(dir.path().join("other.txt"), "other\n").unwrap();
    let original = fs::read(git.join("index")).unwrap();
    {
        let holder = git2::Repository::open(dir.path()).unwrap();
        let mut index = holder.index().unwrap();
        index.add_path(Path::new("other.txt")).unwrap();
        index.write().unwrap();
    }
    let published = fs::read(git.join("index")).unwrap();
    fs::write(git.join("index"), &original).unwrap();
    fs::write(git.join("index.lock"), b"").unwrap();

    let holder_git = git.clone();
    set_index_update_hook(LockHook::BeforeTake, move || {
        fs::write(holder_git.join("index.new"), &published).unwrap();
        fs::rename(holder_git.join("index.new"), holder_git.join("index")).unwrap();
        fs::remove_file(holder_git.join("index.lock")).unwrap();
    });
    fs::write(dir.path().join("file.txt"), "two\n").unwrap();
    stage_and_commit(&repo, &[], Path::new("file.txt"), None, &sig, "second")
        .unwrap()
        .expect("second commit");

    let after = git2::Repository::open(dir.path()).unwrap();
    let index = after.index().unwrap();
    assert!(
        index.get_path(Path::new("other.txt"), 0).is_some(),
        "the lock holder's staged path was dropped"
    );
    let head = after.head().unwrap().peel_to_tree().unwrap();
    let committed = head.get_path(Path::new("file.txt")).unwrap().id();
    assert_eq!(
        index.get_path(Path::new("file.txt"), 0).map(|e| e.id),
        Some(committed),
        "the saved path's index entry is not the committed blob"
    );
    assert!(!git.join("index.lock").exists(), "index.lock left behind");
    assert!(
        !git.join("index.signex-scratch").exists(),
        "scratch index left behind"
    );
}

/// Only a sharing violation on Windows is a held lock worth retrying.
#[test]
fn a_rename_is_retried_only_for_a_windows_sharing_violation() {
    use super::index_lock::{RenameFailure, rename_failure};
    use std::io::ErrorKind;
    assert_eq!(
        rename_failure(ErrorKind::PermissionDenied, true),
        RenameFailure::Locked
    );
    assert_eq!(
        rename_failure(ErrorKind::PermissionDenied, false),
        RenameFailure::Failed
    );
    assert_eq!(
        rename_failure(ErrorKind::NotFound, true),
        RenameFailure::Failed
    );
    assert_eq!(
        rename_failure(ErrorKind::Other, false),
        RenameFailure::Failed
    );
}

/// An edit that panics leaves no `index.lock` behind and the repository on
/// its own index, with the half-made edit dropped.
#[test]
fn a_panicking_edit_releases_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    commit_files(&repo, "HEAD", &[], &[("file.txt", "one\n")]);
    fs::write(dir.path().join("new.txt"), "new\n").unwrap();

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::index_lock::update_index_locked(&repo, |index| -> Result<(), git2::Error> {
            index.add_path(Path::new("new.txt")).unwrap();
            panic!("edit panics")
        })
    }));

    assert!(panicked.is_err());
    assert!(!repo.path().join("index.lock").exists(), "index.lock left");
    assert!(!repo.path().join("index.signex-scratch").exists());
    let index = repo.index().unwrap();
    assert_eq!(index.path(), Some(repo.path().join("index").as_path()));
    assert!(index.get_path(Path::new("new.txt"), 0).is_none());
}

/// A repo at a fresh temp dir with `config` set, opened again so its index
/// takes the capabilities that config names.
fn repo_with_config(config: &[(&str, bool)]) -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let mut cfg = repo.config().unwrap();
    for (key, value) in config {
        cfg.set_bool(key, *value).unwrap();
    }
    let repo = git2::Repository::open(dir.path()).unwrap();
    (dir, repo)
}

fn index_paths(root: &Path) -> Vec<String> {
    let repo = git2::Repository::open(root).unwrap();
    let index = repo.index().unwrap();
    index
        .iter()
        .map(|e| String::from_utf8_lossy(&e.path).into_owned())
        .collect()
}

/// With `core.ignorecase`, a save naming a tracked file in another case
/// updates its entry, kept in its original case, as `git add` does.
#[test]
fn with_ignorecase_a_case_variant_save_keeps_one_entry_in_its_original_case() {
    let (dir, repo) = repo_with_config(&[("core.ignorecase", true)]);
    commit_files(&repo, "HEAD", &[], &[("Foo.txt", "one\n")]);
    fs::write(dir.path().join("foo.txt"), "two\n").unwrap();

    let sig = signature();
    stage_and_commit_with(
        &repo,
        &[],
        None,
        None,
        &[Path::new("foo.txt")],
        &sig,
        "save",
    )
    .unwrap()
    .expect("a commit");

    let variants: Vec<String> = index_paths(dir.path())
        .into_iter()
        .filter(|p| p.eq_ignore_ascii_case("foo.txt"))
        .collect();
    assert_eq!(variants, ["Foo.txt"]);
}

/// With `core.ignorecase`, removing a tracked file named in another case
/// drops its entry.
#[test]
fn with_ignorecase_a_removal_in_another_case_drops_the_entry() {
    let (dir, repo) = repo_with_config(&[("core.ignorecase", true)]);
    commit_files(
        &repo,
        "HEAD",
        &[],
        &[("Foo.txt", "one\n"), ("bar.txt", "one\n")],
    );
    fs::write(dir.path().join("bar.txt"), "two\n").unwrap();

    let sig = signature();
    let removed = [Path::new("foo.txt")];
    stage_and_commit_with(
        &repo,
        &removed,
        Some(Path::new("bar.txt")),
        None,
        &[],
        &sig,
        "move",
    )
    .unwrap()
    .expect("a commit");

    let left: Vec<String> = index_paths(dir.path())
        .into_iter()
        .filter(|p| p.eq_ignore_ascii_case("foo.txt"))
        .collect();
    assert!(
        left.is_empty(),
        "the removed path is still staged: {left:?}"
    );
}

/// With `core.filemode=false`, a save keeps an executable entry's mode
/// rather than taking it from the file on disk.
#[test]
fn with_filemode_off_a_save_keeps_the_executable_mode() {
    let (dir, repo) = repo_with_config(&[("core.filemode", false)]);
    fs::write(dir.path().join("run.sh"), "one\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("run.sh")).unwrap();
    let mut entry = index.get_path(Path::new("run.sh"), 0).unwrap();
    entry.mode = 0o100755;
    index.add(&entry).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = signature();
    repo.commit(Some("HEAD"), &sig, &sig, "exec", &tree, &[])
        .unwrap();
    fs::write(dir.path().join("run.sh"), "two\n").unwrap();

    stage_and_commit_with(&repo, &[], None, None, &[Path::new("run.sh")], &sig, "save")
        .unwrap()
        .expect("a commit");

    let after = git2::Repository::open(dir.path()).unwrap();
    let mode = after
        .index()
        .unwrap()
        .get_path(Path::new("run.sh"), 0)
        .unwrap()
        .mode;
    assert_eq!(mode, 0o100755, "mode {mode:o}");
}

/// After a save the repository handle that made it still has its own
/// index, and sees the saved file as clean.
#[test]
fn a_save_leaves_the_repository_on_its_own_index() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let sig = signature();
    fs::write(dir.path().join("file.txt"), "one\n").unwrap();
    stage_and_commit(&repo, &[], Path::new("file.txt"), None, &sig, "first")
        .unwrap()
        .expect("a commit");

    let index = repo.index().unwrap();
    assert_eq!(index.path(), Some(repo.path().join("index").as_path()));
    assert_eq!(
        repo.status_file(Path::new("file.txt")).unwrap(),
        git2::Status::CURRENT
    );
}

/// The published index keeps a tree cache that still matches: a tree
/// written from it is the commit's tree after a save that changes, adds
/// and removes files.
#[test]
fn the_published_index_writes_the_committed_tree() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    fs::create_dir_all(dir.path().join("dir")).unwrap();
    commit_files(
        &repo,
        "HEAD",
        &[],
        &[
            ("dir/a.txt", "a\n"),
            ("dir/b.txt", "b\n"),
            ("top.txt", "t\n"),
        ],
    );
    // Store the tree cache in the on-disk index.
    let mut index = repo.index().unwrap();
    index.write_tree().unwrap();
    index.write().unwrap();
    fs::write(dir.path().join("dir/a.txt"), "a2\n").unwrap();
    fs::write(dir.path().join("dir/c.txt"), "c\n").unwrap();

    let sig = signature();
    stage_and_commit_with(
        &repo,
        &[Path::new("top.txt")],
        Some(Path::new("dir/a.txt")),
        None,
        &[Path::new("dir/c.txt")],
        &sig,
        "save",
    )
    .unwrap()
    .expect("a commit");

    let committed = repo.head().unwrap().peel_to_tree().unwrap().id();
    let mut published = git2::Index::open(&repo.path().join("index")).unwrap();
    assert_eq!(published.write_tree_to(&repo).unwrap(), committed);
}

/// A save of files left conflicted by a merge resolves them in the index,
/// for the saved path and for an extra path committed with it.
#[test]
fn a_save_of_conflicted_paths_leaves_no_unmerged_stages() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let both = |text| [("a.txt", text), ("b.txt", text)];
    let base_id = commit_files(&repo, "HEAD", &[], &both("base\n"));
    let base = repo.find_commit(base_id).unwrap();
    commit_files(&repo, "HEAD", &[&base], &both("ours\n"));
    // Theirs, built from base without touching the working tree.
    let mut builder = repo.treebuilder(Some(&base.tree().unwrap())).unwrap();
    let theirs_blob = repo.blob(b"theirs\n").unwrap();
    builder.insert("a.txt", theirs_blob, 0o100644).unwrap();
    builder.insert("b.txt", theirs_blob, 0o100644).unwrap();
    let theirs_tree = repo.find_tree(builder.write().unwrap()).unwrap();
    let sig = signature();
    let theirs = repo
        .commit(
            Some("refs/heads/theirs"),
            &sig,
            &sig,
            "theirs",
            &theirs_tree,
            &[&base],
        )
        .unwrap();
    let annotated = repo.find_annotated_commit(theirs).unwrap();
    repo.merge(&[&annotated], None, None).unwrap();
    assert!(repo.index().unwrap().has_conflicts(), "the merge conflicts");

    fs::write(dir.path().join("a.txt"), "resolved a\n").unwrap();
    fs::write(dir.path().join("b.txt"), "resolved b\n").unwrap();
    stage_and_commit_with(
        &repo,
        &[],
        Some(Path::new("a.txt")),
        None,
        &[Path::new("b.txt")],
        &sig,
        "resolve",
    )
    .unwrap()
    .expect("a commit");

    let after = git2::Repository::open(dir.path()).unwrap();
    let index = after.index().unwrap();
    let unmerged: Vec<String> = index
        .iter()
        .filter(|e| (e.flags >> 12) & 0x3 != 0)
        .map(|e| String::from_utf8_lossy(&e.path).into_owned())
        .collect();
    assert!(unmerged.is_empty(), "unmerged stages left: {unmerged:?}");
    for name in ["a.txt", "b.txt"] {
        let status = after.status_file(Path::new(name)).unwrap();
        assert!(
            !status.contains(git2::Status::CONFLICTED),
            "{name} still reads as conflicted: {status:?}"
        );
    }
}
