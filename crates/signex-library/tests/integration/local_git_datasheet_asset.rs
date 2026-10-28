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
        description: String::new(),
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

/// #1471 -- after a save whose commit and restore both failed, the file is on
/// disk, untracked, and the `.snxlib` pins it. A later save of that row
/// (nothing staged, no asset passed) commits the file with it.
#[test]
fn a_save_commits_an_untracked_pdf_the_row_pins() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let seed = row(library_id);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    let hash = store_datasheet(&file, PDF).unwrap();
    let rel = datasheet_asset_rel_path(&hash).unwrap();
    let mut pinned = adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    pinned.datasheet = DatasheetRef::hash_pinned(hash, "ds.pdf");

    // An edit with no asset passed.
    adapter.update_row(TABLE, pinned, "edit").unwrap();

    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    assert!(tree.get_path(Path::new(&rel)).is_ok(), "{rel} not in HEAD");
}

/// A pin whose file is nowhere is left alone: the save neither fails nor
/// invents a file.
#[test]
fn a_pin_with_no_file_anywhere_does_not_block_the_save() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let seed = row(library_id);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    let mut pinned = adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    let hash = signex_library::datasheet_hash(b"%PDF-1.7 never stored");
    pinned.datasheet = DatasheetRef::hash_pinned(hash.clone(), "ds.pdf");

    adapter.update_row(TABLE, pinned, "edit").unwrap();

    assert!(!datasheet_asset_path(&file, &hash).unwrap().exists());
}

#[test]
fn a_failed_commit_with_an_asset_is_an_error_and_leaves_the_row_unsaved() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let seed = row(library_id);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    let mut edited = adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    let hash = store_datasheet(&file, PDF).unwrap();
    edited.datasheet = DatasheetRef::hash_pinned(hash.clone(), "ds.pdf");
    let rel = datasheet_asset_rel_path(&hash).unwrap();
    let before = std::fs::read(&file).unwrap();

    // A repository that cannot be opened makes the commit fail.
    let head = file.parent().unwrap().join(".git").join("HEAD");
    let good_head = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"not a ref").unwrap();
    let err = adapter
        .update_row_with_assets(TABLE, edited.clone(), "pin", std::slice::from_ref(&rel))
        .unwrap_err();
    assert!(err.to_string().contains("not saved"), "{err}");
    assert_eq!(std::fs::read(&file).unwrap(), before, "file restored");
    let stored = adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    assert!(stored.datasheet.is_empty(), "the row is not pinned");

    // Once git works again the same save succeeds and commits the PDF.
    std::fs::write(&head, good_head).unwrap();
    adapter
        .update_row_with_assets(TABLE, edited, "pin", std::slice::from_ref(&rel))
        .unwrap();
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    assert!(tree.get_path(Path::new(&rel)).is_ok());
}

fn head_tree_has(file: &Path, rel: &str) -> bool {
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    tree.get_path(Path::new(rel)).is_ok()
}

/// Seed rows A and B, pin A with its PDF on disk, then move HEAD back to
/// before the pin while disk keeps it (the state a double-failed save
/// leaves). Returns A's pin path and row B.
fn dangling_pin_of_row_a(
    adapter: &LocalGitAdapter,
    file: &Path,
    library_id: Uuid,
    pdf: &[u8],
) -> (String, ComponentRow) {
    let mut a = row(library_id);
    a.internal_pn = InternalPn::new("R-A");
    let mut b = row(library_id);
    b.internal_pn = InternalPn::new("R-B");
    adapter.insert_row(TABLE, a.clone(), "seed a").unwrap();
    adapter.insert_row(TABLE, b.clone(), "seed b").unwrap();
    let mut pinned = adapter.read_row(TABLE, a.row_id.into()).unwrap();
    let hash = store_datasheet(file, pdf).unwrap();
    pinned.datasheet = DatasheetRef::hash_pinned(hash.clone(), "ds.pdf");
    let rel = datasheet_asset_rel_path(&hash).unwrap();
    adapter
        .update_row_with_assets(TABLE, pinned, "pin a", std::slice::from_ref(&rel))
        .unwrap();
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let before_pin = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .parent(0)
        .unwrap();
    repo.reset(before_pin.as_object(), git2::ResetType::Mixed, None)
        .unwrap();
    assert!(!head_tree_has(file, &rel), "HEAD is before the pin");
    assert!(file.parent().unwrap().join(&rel).is_file());
    let b = adapter.read_row(TABLE, b.row_id.into()).unwrap();
    (rel, b)
}

/// #1471 -- every `.snxlib` commit carries every row's pins, so saving a
/// different row also commits another row's untracked pinned PDF.
#[test]
fn saving_another_row_commits_an_untracked_pdf_a_row_pins() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let (rel, mut b) = dangling_pin_of_row_a(&adapter, &file, library_id, PDF);

    b.datasheet_label = "edited".into();
    adapter.update_row(TABLE, b, "edit b").unwrap();

    assert!(head_tree_has(&file, &rel), "{rel} not in HEAD");
}

#[test]
fn inserting_a_row_commits_an_untracked_pdf_a_row_pins() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let (rel, _) = dangling_pin_of_row_a(&adapter, &file, library_id, PDF);

    let mut c = row(library_id);
    c.internal_pn = InternalPn::new("R-C");
    adapter.insert_row(TABLE, c, "insert c").unwrap();

    assert!(head_tree_has(&file, &rel), "{rel} not in HEAD");
}

/// A file whose bytes are not what its name claims is not committed under
/// that content address.
#[test]
fn a_pinned_file_with_the_wrong_bytes_is_not_committed() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let (rel, mut b) = dangling_pin_of_row_a(&adapter, &file, library_id, PDF);
    std::fs::write(file.parent().unwrap().join(&rel), b"%PDF-1.7 overwritten").unwrap();

    b.datasheet_label = "edited".into();
    adapter.update_row(TABLE, b, "edit b").unwrap();

    assert!(
        !head_tree_has(&file, &rel),
        "a mismatched file was committed"
    );
}

/// #1522 -- the pin scan skips rows whose `datasheet` cell cannot pin a PDF.
/// A pin in a further document (a labelled list cell) must still count, and a
/// row with a plain URL beside it must not hide it.
#[test]
fn a_pin_in_a_further_document_is_committed_beside_url_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let mut url_row = row(library_id);
    url_row.internal_pn = InternalPn::new("R-URL");
    url_row.datasheet = DatasheetRef::url("https://example.com/ds.pdf");
    adapter.insert_row(TABLE, url_row, "seed url").unwrap();
    let seed = row(library_id);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();

    let hash = store_datasheet(&file, PDF).unwrap();
    let rel = datasheet_asset_rel_path(&hash).unwrap();
    let mut pinned = adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    pinned.more_datasheets = vec![signex_library::component::DatasheetDoc::new(
        "Errata",
        DatasheetRef::hash_pinned(hash, "errata.pdf"),
    )];

    adapter.update_row(TABLE, pinned, "pin errata").unwrap();

    assert!(head_tree_has(&file, &rel), "{rel} not in HEAD");
}

/// #1522 -- a hand-edited cell that JSON-escapes the pin's kind still pins
/// the PDF, so the cheap prefilter must not skip it.
#[test]
fn an_escaped_pin_kind_is_still_committed_on_another_rows_save() {
    let tmp = tempfile::tempdir().unwrap();
    let (file, adapter, library_id) = mounted_library(tmp.path(), true);
    let (rel, mut b) = dangling_pin_of_row_a(&adapter, &file, library_id, PDF);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("hash_pinned"));
    std::fs::write(&file, text.replace("hash_pinned", "hash\\u005fpinned")).unwrap();

    b.datasheet_label = "edited".into();
    adapter.update_row(TABLE, b, "edit b").unwrap();

    assert!(head_tree_has(&file, &rel), "{rel} not in HEAD");
}
