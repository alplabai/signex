//! Issue #831 — a failed encode or write must leave the in-memory
//! `LibraryFile` and the `.snxlib` on disk untouched. A failed git commit
//! must NOT roll the edit back: it is kept (memory == disk), a warning is
//! surfaced, and the next successful commit includes it.

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

/// The index must hold either the last committed content or the content
/// on disk — never a third state (a rolled-back edit).
fn assert_index_agrees_with_disk(root: &Path, file: &Path) {
    let rel = file.file_name().unwrap().to_str().unwrap();
    let disk =
        git2::Oid::hash_object(git2::ObjectType::Blob, &std::fs::read(file).unwrap()).unwrap();
    let staged = index_blob(root, rel);
    assert!(
        staged == Some(disk) || staged == head_blob(root, rel),
        "index stages content that is neither HEAD nor disk"
    );
}

#[test]
fn failed_commit_keeps_the_edit_and_warns() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    let root = dir.path().join("Rb");
    adapter.add_library_class(class("res"), "add res").unwrap();

    let head = root.join(".git").join("HEAD");
    let good_head = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"garbage\n").unwrap();
    adapter
        .add_library_class(class("cap"), "add cap")
        .expect("a commit failure must not reject the edit");
    assert_eq!(
        disk_classes(&file),
        vec!["res", "cap"],
        "edit stays on disk"
    );
    assert_eq!(memory_classes(&adapter), disk_classes(&file));
    let warnings = adapter.take_commit_warnings();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with("saved, but the git commit failed:"));
    assert!(warnings[0].ends_with("it will be included in the next commit"));
    assert_index_agrees_with_disk(&root, &file);

    std::fs::write(&head, good_head).unwrap();
    adapter.add_library_class(class("ind"), "add ind").unwrap();
    assert!(adapter.take_commit_warnings().is_empty());
    assert_eq!(disk_classes(&file), vec!["res", "cap", "ind"]);
}

#[test]
fn index_lock_keeps_edit_and_next_commit_includes_both() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    let root = dir.path().join("Rb");
    adapter.add_library_class(class("res"), "add res").unwrap();

    let lock = root.join(".git").join("index.lock");
    std::fs::write(&lock, b"").unwrap();
    adapter.add_library_class(class("cap"), "add cap").unwrap();
    assert_eq!(disk_classes(&file), vec!["res", "cap"]);
    assert_eq!(memory_classes(&adapter), disk_classes(&file));
    assert_eq!(adapter.take_commit_warnings().len(), 1);
    assert_index_agrees_with_disk(&root, &file);

    std::fs::remove_file(&lock).unwrap();
    adapter.add_library_class(class("ind"), "add ind").unwrap();
    assert!(adapter.take_commit_warnings().is_empty());
    assert_eq!(memory_classes(&adapter), disk_classes(&file));
    // HEAD's `.snxlib` carries both the held-back and the new edit.
    let repo = git2::Repository::open(&root).unwrap();
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
    let committed = LibraryFile::parse(std::str::from_utf8(blob.content()).unwrap()).unwrap();
    let keys: Vec<_> = committed
        .manifest
        .classes
        .into_iter()
        .map(|c| c.key)
        .collect();
    assert_eq!(keys, vec!["res", "cap", "ind"]);
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
