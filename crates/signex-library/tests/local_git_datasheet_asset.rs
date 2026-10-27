//! #1332 -- a pinned datasheet PDF is committed together with the row that
//! pins it, so the hash resolves in a clone of a git-backed library.

#![cfg(feature = "local-git")]

use std::path::{Path, PathBuf};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use signex_library::{datasheet_asset_path, datasheet_asset_rel_path, store_datasheet};
use uuid::Uuid;

const TABLE: &str = "R";
const PDF: &[u8] = b"%PDF-1.7 pinned datasheet bytes";

fn mounted_library(dir: &Path, enable_git: bool) -> (PathBuf, LocalGitAdapter, Uuid) {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Pin").join("Pin.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Pin".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git,
            use_lfs: false,
        },
    )
    .expect("init succeeds");
    (file, adapter, library_id)
}

fn row(library_id: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R-1332"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "R-1332"),
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

/// Seed one row, then pin `PDF` on it: write the file, then save the row
/// with the asset. Returns the pinned hash.
fn pin(adapter: &LocalGitAdapter, file: &Path, library_id: Uuid) -> (ComponentRow, String) {
    let seed = row(library_id);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    let mut edited = adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    let hash = store_datasheet(file, PDF).unwrap();
    edited.datasheet = DatasheetRef::hash_pinned(hash.clone(), "ds.pdf");
    let rel = datasheet_asset_rel_path(&hash).unwrap();
    adapter
        .update_row_with_assets(TABLE, edited.clone(), "pin datasheet", &[rel])
        .unwrap();
    (edited, hash)
}

#[test]
fn the_pinned_pdf_is_in_the_same_commit_as_its_row() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let (_, hash) = pin(&adapter, &file, library_id);

    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap().trim(), "pin datasheet");
    let tree = head.tree().unwrap();
    let rel = datasheet_asset_rel_path(&hash).unwrap();
    assert!(tree.get_path(Path::new(&rel)).is_ok(), "asset in HEAD");
    assert!(tree.get_path(Path::new("Pin.snxlib")).is_ok());
    // Nothing is left untracked or modified.
    let statuses = repo.statuses(None).unwrap();
    assert!(
        statuses.iter().all(|s| s.status().is_ignored()),
        "working tree is clean after the save"
    );
}

#[test]
fn a_clone_resolves_the_pinned_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let (edited, hash) = pin(&adapter, &file, library_id);

    let clone_dir = tmp.path().join("clone");
    git2::Repository::clone(file.parent().unwrap().to_str().unwrap(), &clone_dir).unwrap();
    let cloned_file = clone_dir.join("Pin.snxlib");

    let cloned = LocalGitAdapter::open(&cloned_file).unwrap();
    let back = cloned.read_row(TABLE, edited.row_id.into()).unwrap();
    let DatasheetRef::HashPinned { hash: pinned, .. } = back.datasheet else {
        panic!("the cloned row is not pinned");
    };
    assert_eq!(pinned, hash);
    let path = datasheet_asset_path(&cloned_file, &pinned).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), PDF);
}

#[test]
fn an_asset_that_is_not_a_written_datasheet_is_refused_before_any_write() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let seed = row(library_id);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    let edited = adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    let before = std::fs::read(&file).unwrap();

    let hash = signex_library::datasheet_hash(PDF);
    for rel in [
        // Valid name, never written.
        datasheet_asset_rel_path(&hash).unwrap(),
        // Not a datasheet path at all.
        "Pin.snxlib".to_string(),
        "assets/datasheets/../../secret.pdf".to_string(),
    ] {
        let err = adapter
            .update_row_with_assets(TABLE, edited.clone(), "pin", std::slice::from_ref(&rel))
            .unwrap_err();
        assert!(!err.to_string().is_empty(), "{rel}");
    }
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn a_library_without_git_saves_the_row_and_keeps_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), false);
    let (edited, hash) = pin(&adapter, &file, library_id);
    let back = adapter.read_row(TABLE, edited.row_id.into()).unwrap();
    assert_eq!(
        back.datasheet,
        DatasheetRef::hash_pinned(hash.clone(), "ds.pdf")
    );
    assert!(datasheet_asset_path(&file, &hash).unwrap().is_file());
}
