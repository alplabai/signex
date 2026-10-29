//! #1121 — files that `.gitattributes` routes through Git LFS are
//! committed as LFS pointers with their content in `.git/lfs/objects/`,
//! not as raw blobs. libgit2 runs no `git-lfs` filter, so Signex writes
//! the pointers itself.

#![cfg(feature = "local-git")]

use std::fs;
use std::path::Path;

use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use signex_library::enable_project_version_control;
use tempfile::TempDir;

const MODEL: &[u8] = b"ISO-10303-21;\nHEADER;\n/* a STEP model */\nENDSEC;\n";

fn committed(dir: &Path, rel: &str) -> Vec<u8> {
    let repo = git2::Repository::open(dir).unwrap();
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    let entry = tree.get_path(Path::new(rel)).unwrap();
    repo.find_blob(entry.id()).unwrap().content().to_vec()
}

fn staged(dir: &Path, rel: &str) -> git2::Oid {
    let repo = git2::Repository::open(dir).unwrap();
    let index = repo.index().unwrap();
    index.get_path(Path::new(rel), 0).unwrap().id
}

fn lfs_object(dir: &Path, pointer: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(pointer.to_vec()).unwrap();
    let oid = text
        .lines()
        .find_map(|l| l.strip_prefix("oid sha256:"))
        .expect("pointer names its object");
    fs::read(
        dir.join(".git/lfs/objects")
            .join(&oid[0..2])
            .join(&oid[2..4])
            .join(oid),
    )
    .expect("the LFS object is stored")
}

fn project_with_model() -> TempDir {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("board.snxprj"), "[project]\n").unwrap();
    fs::create_dir_all(tmp.path().join("models")).unwrap();
    fs::write(tmp.path().join("models/part.step"), MODEL).unwrap();
    tmp
}

#[test]
fn enabling_with_lfs_commits_models_as_pointers() {
    let tmp = project_with_model();

    enable_project_version_control(tmp.path(), true, true, None).unwrap();

    let pointer = committed(tmp.path(), "models/part.step");
    assert!(
        pointer.starts_with(b"version https://git-lfs.github.com/spec/v1\n"),
        "committed as a raw blob: {:?}",
        String::from_utf8_lossy(&pointer)
    );
    assert!(
        String::from_utf8_lossy(&pointer).contains(&format!("size {}\n", MODEL.len())),
        "the pointer records the size"
    );
    assert_eq!(lfs_object(tmp.path(), &pointer), MODEL);
    // The index agrees with the commit, so `git status` is clean.
    let repo = git2::Repository::open(tmp.path()).unwrap();
    let head = repo.head().unwrap().peel_to_tree().unwrap();
    assert_eq!(
        staged(tmp.path(), "models/part.step"),
        head.get_path(Path::new("models/part.step")).unwrap().id()
    );
    // Non-LFS files are unchanged, and the working copy keeps the model.
    assert_eq!(committed(tmp.path(), "board.snxprj"), b"[project]\n");
    assert_eq!(
        fs::read(tmp.path().join("models/part.step")).unwrap(),
        MODEL
    );
    // `git push` will upload the objects (or say git-lfs is missing).
    let hook = fs::read_to_string(tmp.path().join(".git/hooks/pre-push")).unwrap();
    assert!(hook.contains("git lfs pre-push"), "{hook}");
}

#[test]
fn enabling_without_lfs_commits_models_as_they_are() {
    let tmp = project_with_model();

    enable_project_version_control(tmp.path(), false, true, None).unwrap();

    assert_eq!(committed(tmp.path(), "models/part.step"), MODEL);
    assert!(!tmp.path().join(".git/lfs").exists());
    assert!(!tmp.path().join(".git/hooks/pre-push").exists());
}

#[test]
fn a_later_commit_of_an_lfs_path_records_a_pointer() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    fs::write(
        root.join(".gitattributes"),
        "*.step filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    fs::write(root.join("part.step"), MODEL).unwrap();

    adapter
        .commit_path(Path::new("part.step"), "Add part")
        .unwrap()
        .expect("a commit");

    let pointer = committed(&root, "part.step");
    assert!(pointer.starts_with(b"version https://git-lfs.github.com/spec/v1\n"));
    assert_eq!(lfs_object(&root, &pointer), MODEL);
    let repo = git2::Repository::open(&root).unwrap();
    let head = repo.head().unwrap().peel_to_tree().unwrap();
    assert_eq!(
        staged(&root, "part.step"),
        head.get_path(Path::new("part.step")).unwrap().id(),
        "the index records the pointer that was committed"
    );
}

#[test]
fn enabling_with_lfs_before_any_model_still_installs_the_push_hook() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("board.snxprj"), "[project]\n").unwrap();

    enable_project_version_control(tmp.path(), true, true, None).unwrap();
    assert!(tmp.path().join(".git/hooks/pre-push").exists());
}

#[test]
fn a_model_committed_after_an_lfs_enable_gets_a_push_hook() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    fs::write(
        root.join(".gitattributes"),
        "*.step filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    fs::write(root.join("part.step"), MODEL).unwrap();
    assert!(!root.join(".git/hooks/pre-push").exists());

    adapter
        .commit_path(Path::new("part.step"), "Add part")
        .unwrap()
        .expect("a commit");

    assert!(root.join(".git/hooks/pre-push").exists());
}

#[test]
fn committing_bytes_for_an_lfs_path_records_the_pointer_of_those_bytes_not_the_files() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    fs::write(
        root.join(".gitattributes"),
        "*.step filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    // The working file is overwritten after the bytes were handed over.
    fs::write(root.join("part.step"), b"edited later").unwrap();

    adapter
        .commit_path_with_bytes(Path::new("part.step"), MODEL, "Save part.step")
        .unwrap()
        .expect("a commit");

    let pointer = committed(&root, "part.step");
    assert!(pointer.starts_with(b"version https://git-lfs.github.com/spec/v1\n"));
    assert_eq!(
        lfs_object(&root, &pointer),
        MODEL,
        "the object is the saved bytes"
    );
    assert_eq!(fs::read(root.join("part.step")).unwrap(), b"edited later");
    let hook = fs::read_to_string(root.join(".git/hooks/pre-push")).unwrap();
    assert!(hook.contains("git lfs pre-push"), "{hook}");
}

#[test]
fn an_lfs_byte_commit_whose_file_changed_afterwards_leaves_it_reported_modified() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    fs::write(
        root.join(".gitattributes"),
        "*.step filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    fs::write(root.join("part.step"), b"a different model").unwrap();
    // As a file written long before the commit: git trusts the stat cache
    // only for entries older than the index.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    fs::File::options()
        .write(true)
        .open(root.join("part.step"))
        .unwrap()
        .set_modified(old)
        .unwrap();

    adapter
        .commit_path_with_bytes(Path::new("part.step"), MODEL, "Save part.step")
        .unwrap()
        .expect("a commit");

    let repo = git2::Repository::open(&root).unwrap();
    let status = repo.status_file(Path::new("part.step")).unwrap();
    assert!(
        status.contains(git2::Status::WT_MODIFIED),
        "git must see the file differs from the pointer, got {status:?}"
    );
}

#[test]
fn an_lfs_byte_commit_whose_file_still_matches_leaves_it_clean() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    fs::write(
        root.join(".gitattributes"),
        "*.step filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    fs::write(root.join("part.step"), MODEL).unwrap();

    adapter
        .commit_path_with_bytes(Path::new("part.step"), MODEL, "Save part.step")
        .unwrap()
        .expect("a commit");

    let repo = git2::Repository::open(&root).unwrap();
    let head = repo.head().unwrap().peel_to_tree().unwrap();
    assert_eq!(
        staged(&root, "part.step"),
        head.get_path(Path::new("part.step")).unwrap().id(),
        "the index records the pointer that was committed"
    );
}
