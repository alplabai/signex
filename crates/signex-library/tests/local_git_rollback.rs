//! Issue #831 — a failed encode or write must leave the in-memory
//! `LibraryFile` and the `.snxlib` on disk untouched. A failed git commit
//! must NOT roll the edit back: it is kept (memory == disk) and logged. Until
//! the next library commit the git index holds exactly HEAD's `.snxlib`, so a
//! commit through any other path never stores the uncommitted edit.

#![cfg(feature = "local-git")]

use std::path::{Path, PathBuf};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{
    ClassEntry, FORMAT_TOKEN, LibraryFile, LibrarySection, SnxlibManifest,
};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

fn init(dir: &tempfile::TempDir) -> (PathBuf, Uuid, LocalGitAdapter) {
    signex_library::git_identity::test_identity::install();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "Rb".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let lib_id = manifest.library_id;
    let file = dir.path().join("Rb").join("Rb.snxlib");
    let adapter = LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init");
    (file, lib_id, adapter)
}

fn row(pn: &str, lib_id: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib_id, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn class(key: &str) -> ClassEntry {
    ClassEntry {
        key: key.into(),
        label: key.into(),
    }
}

fn disk_classes(file: &Path) -> Vec<String> {
    let lf = LibraryFile::parse(&std::fs::read_to_string(file).unwrap()).unwrap();
    lf.manifest.classes.into_iter().map(|c| c.key).collect()
}

fn memory_classes(a: &LocalGitAdapter) -> Vec<String> {
    a.library_classes().into_iter().map(|c| c.key).collect()
}

#[test]
fn failed_write_leaves_memory_equal_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    adapter.add_library_class(class("res"), "add res").unwrap();
    let good = std::fs::read(&file).unwrap();

    // A directory squatting on the target path makes the rename fail.
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(adapter.add_library_class(class("cap"), "add cap").is_err());
    assert_eq!(memory_classes(&adapter), vec!["res".to_string()]);

    // Disk heals; a later valid edit saves, and memory + disk agree.
    std::fs::remove_dir(&file).unwrap();
    std::fs::write(&file, good).unwrap();
    adapter.add_library_class(class("ind"), "add ind").unwrap();
    assert_eq!(disk_classes(&file), memory_classes(&adapter));
    assert_eq!(disk_classes(&file), vec!["res", "ind"]);
}

/// Blob id the index holds for the `.snxlib`, if staged.
fn index_blob(root: &Path, rel: &str) -> Option<git2::Oid> {
    let repo = git2::Repository::open(root).unwrap();
    let index = repo.index().unwrap();
    index.get_path(Path::new(rel), 0).map(|e| e.id)
}

fn head_blob(root: &Path, rel: &str) -> Option<git2::Oid> {
    let repo = git2::Repository::open(root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    tree.get_path(Path::new(rel)).ok().map(|e| e.id())
}

/// Policy: an uncommitted library edit lives on disk and in memory only.
/// The on-disk index never stages it, so a commit through ANY other path
/// stores HEAD's `.snxlib`; only the next library commit (which re-adds the
/// file) picks the edit up.
fn assert_index_is_head(root: &Path, file: &Path) {
    let rel = file.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        index_blob(root, rel),
        head_blob(root, rel),
        "index must hold exactly HEAD's .snxlib while an edit is uncommitted"
    );
}

fn head_oid(root: &Path) -> git2::Oid {
    let repo = git2::Repository::open(root).unwrap();
    repo.head().unwrap().peel_to_commit().unwrap().id()
}

fn head_classes(root: &Path) -> Vec<String> {
    let repo = git2::Repository::open(root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let blob = repo
        .find_blob(tree.get_path(Path::new("Rb.snxlib")).unwrap().id())
        .unwrap();
    let lf = LibraryFile::parse(std::str::from_utf8(blob.content()).unwrap()).unwrap();
    lf.manifest.classes.into_iter().map(|c| c.key).collect()
}

fn head_message(root: &Path) -> String {
    let repo = git2::Repository::open(root).unwrap();
    let commit = repo.head().unwrap().peel_to_commit().unwrap();
    commit.message().unwrap().to_string()
}

/// Make the next `repo.commit` fail AFTER the tree was built: a lock file
/// on the branch ref. Returns the lock path; remove it to repair.
fn lock_branch_ref(root: &Path) -> PathBuf {
    let head = std::fs::read_to_string(root.join(".git").join("HEAD")).unwrap();
    let branch_ref = head.trim().strip_prefix("ref: ").unwrap().to_string();
    let lock = root.join(".git").join(format!("{branch_ref}.lock"));
    std::fs::write(&lock, b"").unwrap();
    lock
}

/// Commit an unrelated file through the adapter's generic commit path.
fn commit_other_file(adapter: &LocalGitAdapter, root: &Path, msg: &str) {
    let other = root.join("notes.txt");
    std::fs::write(&other, msg).unwrap();
    adapter.commit_external_change(&other, msg).unwrap();
}

#[test]
fn late_commit_failure_keeps_edit_unstaged_and_other_commits_exclude_it() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    let root = dir.path().join("Rb");
    adapter.add_library_class(class("res"), "add res").unwrap();
    let head_before = head_oid(&root);

    let lock = lock_branch_ref(&root);
    adapter
        .add_library_class(class("cap"), "add cap")
        .expect("a commit failure must not reject the edit");
    assert_eq!(disk_classes(&file), vec!["res", "cap"]);
    assert_eq!(memory_classes(&adapter), disk_classes(&file));
    std::fs::remove_file(&lock).unwrap();
    assert_eq!(head_oid(&root), head_before, "HEAD must not move");
    assert_index_is_head(&root, &file);

    // Another path's commit must not sweep the library edit in.
    commit_other_file(&adapter, &root, "unrelated");
    assert_eq!(head_message(&root).trim(), "unrelated");
    assert_eq!(head_classes(&root), vec!["res"]);

    // The next library commit stores the current disk content.
    adapter.add_library_class(class("ind"), "add ind").unwrap();
    assert_eq!(head_classes(&root), vec!["res", "cap", "ind"]);
}

#[test]
fn corrupt_head_keeps_edit_and_next_library_commit_includes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    let root = dir.path().join("Rb");
    adapter.add_library_class(class("res"), "add res").unwrap();
    let head_before = head_oid(&root);

    let head = root.join(".git").join("HEAD");
    let good_head = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"garbage\n").unwrap();
    adapter
        .add_library_class(class("cap"), "add cap")
        .expect("a commit failure must not reject the edit");
    assert_eq!(disk_classes(&file), vec!["res", "cap"]);
    assert_eq!(memory_classes(&adapter), disk_classes(&file));

    std::fs::write(&head, good_head).unwrap();
    assert_eq!(head_oid(&root), head_before);
    assert_index_is_head(&root, &file);
    commit_other_file(&adapter, &root, "unrelated");
    assert_eq!(head_classes(&root), vec!["res"]);
    adapter.add_library_class(class("ind"), "add ind").unwrap();
    assert_eq!(head_classes(&root), vec!["res", "cap", "ind"]);
}

#[test]
fn two_failures_in_a_row_never_store_a_stale_snxlib() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    let root = dir.path().join("Rb");
    adapter.add_library_class(class("res"), "add res").unwrap();

    // Failure 1: late (branch ref locked after staging).
    let ref_lock = lock_branch_ref(&root);
    adapter.add_library_class(class("cap"), "add cap").unwrap();
    std::fs::remove_file(&ref_lock).unwrap();
    assert_index_is_head(&root, &file);

    // Failure 2: early (HEAD unreadable). A held `index.lock` no longer
    // fails the commit: the commit is built from HEAD's tree and succeeds,
    // and only the best-effort index refresh waits on the lock.
    let head = root.join(".git").join("HEAD");
    let good_head = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"garbage\n").unwrap();
    adapter.add_library_class(class("ind"), "add ind").unwrap();
    std::fs::write(&head, good_head).unwrap();
    assert_eq!(disk_classes(&file), vec!["res", "cap", "ind"]);
    assert_eq!(memory_classes(&adapter), disk_classes(&file));
    assert_index_is_head(&root, &file);

    // A commit through another path: HEAD's .snxlib is still the last
    // committed one, never an intermediate.
    commit_other_file(&adapter, &root, "unrelated");
    assert_eq!(head_classes(&root), vec!["res"]);

    adapter.add_library_class(class("x"), "add x").unwrap();
    assert_eq!(head_classes(&root), vec!["res", "cap", "ind", "x"]);
    assert_eq!(head_message(&root).trim(), "add x");
}

#[test]
fn unencodable_cell_is_rejected_and_library_stays_saveable() {
    let dir = tempfile::tempdir().unwrap();
    let (file, lib_id, adapter) = init(&dir);
    adapter
        .insert_row("resistors", row("R-1", lib_id), "first")
        .unwrap();
    let before = std::fs::read(&file).unwrap();

    for bad in ["R\t2", "R\n2"] {
        assert!(
            adapter
                .insert_row("resistors", row(bad, lib_id), "bad")
                .is_err(),
            "{bad:?} must be rejected"
        );
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert_eq!(adapter.read_table("resistors").unwrap().len(), 1);
    }

    adapter
        .insert_row("resistors", row("R-3", lib_id), "ok")
        .unwrap();
    assert_eq!(adapter.read_table("resistors").unwrap().len(), 2);
    let on_disk = LibraryFile::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(on_disk.tables["resistors"].rows.len(), 2);
}
