//! #858 / #842 review: HEAD moved by another git client or adapter between
//! the commit's parent lookup and the commit itself.

use super::*;
use crate::library_file::{FORMAT_TOKEN, LibrarySection};
use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

fn adapter(dir: &tempfile::TempDir) -> LocalGitAdapter {
    crate::git_identity::test_identity::install();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: uuid::Uuid::now_v7(),
        library: LibrarySection {
            name: "Race".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(
        dir.path().join("Race").join("Race.snxlib"),
        manifest,
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

/// Commit `name` = `content` on top of the current tip with a second git
/// client, without touching the working tree or the index of this repo.
fn external_commit(root: &Path, name: &str, content: &str) -> git2::Oid {
    let repo = git2::Repository::open(root).unwrap();
    let blob = repo.blob(content.as_bytes()).unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    let mut builder = repo.treebuilder(Some(&parent.tree().unwrap())).unwrap();
    builder.insert(name, blob, 0o100644).unwrap();
    let tree = repo.find_tree(builder.write().unwrap()).unwrap();
    let sig = git2::Signature::now("Other", "other@example.test").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "external", &tree, &[&parent])
        .unwrap()
}

fn head_blob(root: &Path, name: &str) -> Option<Vec<u8>> {
    let repo = git2::Repository::open(root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let entry = tree.get_path(Path::new(name)).ok()?;
    Some(repo.find_blob(entry.id()).unwrap().content().to_vec())
}

fn index_blob(root: &Path, name: &str) -> Option<git2::Oid> {
    let repo = git2::Repository::open(root).unwrap();
    let index = repo.index().unwrap();
    index.get_path(Path::new(name), 0).map(|e| e.id)
}

fn head_blob_id(root: &Path, name: &str) -> Option<git2::Oid> {
    let repo = git2::Repository::open(root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    tree.get_path(Path::new(name)).ok().map(|e| e.id())
}

fn aged_ref_lock(root: &Path) -> PathBuf {
    let head = fs::read_to_string(root.join(".git").join("HEAD")).unwrap();
    let branch = head.trim().strip_prefix("ref: ").unwrap().to_owned();
    let lock = root.join(".git").join(format!("{branch}.lock"));
    let file = fs::File::create(&lock).unwrap();
    file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(60))
        .unwrap();
    lock
}

/// HEAD moves after the parent was resolved and the commit still succeeds:
/// libgit2 refuses it as `Modified`, the retry rebuilds on the new tip, and
/// both the other client's file and this save are in HEAD.
#[test]
fn a_head_moved_before_the_commit_loses_neither_change() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = adapter(&dir);
    let root = adapter.root_dir.clone();
    adapter.add_library_class(class("res"), "add res").unwrap();

    let moved = root.clone();
    helpers::set_before_commit_hook(move || {
        external_commit(&moved, "theirs.txt", "their bytes");
    });
    adapter.add_library_class(class("cap"), "add cap").unwrap();

    assert_eq!(adapter.pending_changes(), None);
    assert_eq!(head_blob(&root, "theirs.txt").unwrap(), b"their bytes");
    let on_disk = fs::read(&adapter.file_path).unwrap();
    assert_eq!(head_blob(&root, "Race.snxlib").unwrap(), on_disk);
}

/// HEAD moves and then the commit fails. Nothing is restored from the old
/// parent: the index was never touched, and the next commit is built on the
/// moved tip, so the other client's file is not reverted and this save is
/// recorded.
#[test]
fn a_head_moved_before_a_failing_commit_is_never_reverted_by_the_next_commit() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = adapter(&dir);
    let root = adapter.root_dir.clone();
    adapter.add_library_class(class("res"), "add res").unwrap();
    let index_before = index_blob(&root, "Race.snxlib");

    let moved = root.clone();
    let lock = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot = lock.clone();
    helpers::set_before_commit_hook(move || {
        external_commit(&moved, "theirs.txt", "their bytes");
        *slot.borrow_mut() = Some(aged_ref_lock(&moved));
    });
    adapter
        .add_library_class(class("cap"), "add cap")
        .expect("the save succeeded");
    let lock = lock.borrow_mut().take().expect("the hook ran");

    assert!(adapter.pending_changes().is_some());
    assert_eq!(
        index_blob(&root, "Race.snxlib"),
        index_before,
        "a failed commit leaves the index alone"
    );
    assert_eq!(head_blob(&root, "theirs.txt").unwrap(), b"their bytes");

    fs::remove_file(lock).unwrap();
    adapter.add_library_class(class("ind"), "add ind").unwrap();
    assert_eq!(adapter.pending_changes(), None);
    assert_eq!(
        head_blob(&root, "theirs.txt").unwrap(),
        b"their bytes",
        "the other client's file was reverted"
    );
    let on_disk = fs::read(&adapter.file_path).unwrap();
    assert_eq!(head_blob(&root, "Race.snxlib").unwrap(), on_disk);
    assert_eq!(
        index_blob(&root, "Race.snxlib"),
        head_blob_id(&root, "Race.snxlib")
    );
}

/// A repository with no commit yet: the failed commit leaves it unborn and
/// the next one is the first commit, holding every pending file.
#[test]
fn a_failed_commit_on_an_unborn_head_is_recorded_by_the_first_commit() {
    crate::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Fresh");
    fs::create_dir_all(&root).unwrap();
    git2::Repository::init(&root).unwrap();
    let lib = LocalGitAdapter::init(
        root.join("Fresh.snxlib"),
        SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: uuid::Uuid::now_v7(),
            library: LibrarySection {
                name: "Fresh".into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        },
        LibraryInitOptions::default(),
    )
    .unwrap();

    let lock = aged_ref_lock(&root);
    lib.add_library_class(class("res"), "add res").unwrap();
    assert!(lib.pending_changes().is_some());
    assert!(
        git2::Repository::open(&root).unwrap().head().is_err(),
        "HEAD is still unborn"
    );

    fs::remove_file(lock).unwrap();
    assert_eq!(
        lib.commit_pending_changes("first"),
        CommitOutcome::Committed
    );
    assert_eq!(lib.pending_changes(), None);
    assert_eq!(
        head_blob(&root, "Fresh.snxlib").unwrap(),
        fs::read(root.join("Fresh.snxlib")).unwrap()
    );
}
