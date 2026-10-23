//! Issue #831 — a failed encode, write or commit must leave the in-memory
//! `LibraryFile` equal to the `.snxlib` on disk, and the library saveable.

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

#[test]
fn failed_commit_restores_disk_and_memory() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    adapter.add_library_class(class("res"), "add res").unwrap();
    let before = std::fs::read(&file).unwrap();

    let head = dir.path().join("Rb").join(".git").join("HEAD");
    let good_head = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"garbage\n").unwrap();
    assert!(adapter.add_library_class(class("cap"), "add cap").is_err());
    assert_eq!(std::fs::read(&file).unwrap(), before, "disk rolled back");
    assert_eq!(memory_classes(&adapter), vec!["res".to_string()]);

    std::fs::write(&head, good_head).unwrap();
    adapter.add_library_class(class("cap"), "add cap").unwrap();
    assert_eq!(disk_classes(&file), memory_classes(&adapter));
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
