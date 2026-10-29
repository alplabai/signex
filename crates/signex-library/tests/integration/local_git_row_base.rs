//! #1108 -- a local library row carries a base (a hash of the stored row)
//! and a write based on a stale copy is refused instead of overwriting.

use std::path::{Path, PathBuf};

use signex_library::adapter::{LibraryAdapter, LibraryError};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

fn mounted_library(dir: &Path) -> (PathBuf, LocalGitAdapter, Uuid) {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Ext").join("Ext.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Ext".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(&file, manifest, LibraryInitOptions::default())
        .expect("init succeeds");
    (file, adapter, library_id)
}

fn row(internal_pn: &str, library_id: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(internal_pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", internal_pn),
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

const TABLE: &str = "R";

fn seeded() -> (tempfile::TempDir, LocalGitAdapter, ComponentRow) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_file, adapter, library_id) = mounted_library(tmp.path());
    let seed = row("R-1108", library_id);
    adapter
        .insert_row(TABLE, seed.clone(), "seed")
        .expect("insert");
    let read = adapter.read_row(TABLE, seed.row_id.into()).expect("read");
    (tmp, adapter, read)
}

fn with_pn(row: &ComponentRow, pn: &str) -> ComponentRow {
    let mut r = row.clone();
    r.internal_pn = InternalPn::new(pn);
    r
}

#[test]
fn a_row_is_read_with_a_base() {
    let (_tmp, _adapter, read) = seeded();
    assert!(read.server_revision.is_some());
}

#[test]
fn a_stale_update_is_refused_and_the_newer_value_survives() {
    let (_tmp, adapter, read) = seeded();
    // Another surface commits first, from the same base.
    adapter
        .update_row(TABLE, with_pn(&read, "R-browser"), "cell")
        .expect("first write from the fresh base");
    // The stale copy now tries to save.
    let err = adapter
        .update_row(TABLE, with_pn(&read, "R-preview"), "preview")
        .expect_err("stale base must be refused");
    assert!(matches!(err, LibraryError::RowChanged { .. }), "{err}");
    let stored = adapter.read_row(TABLE, read.row_id.into()).expect("read");
    assert_eq!(stored.internal_pn.as_str(), "R-browser");
}

#[test]
fn sequential_saves_with_the_returned_base_are_not_refused() {
    let (_tmp, adapter, read) = seeded();
    let base = adapter
        .update_row(TABLE, with_pn(&read, "R-a"), "one")
        .expect("first");
    let mut second = with_pn(&read, "R-b");
    second.server_revision = base;
    let base = adapter
        .update_row(TABLE, second.clone(), "two")
        .expect("second");
    // The returned base equals what a fresh read now carries.
    let fresh = adapter.read_row(TABLE, read.row_id.into()).expect("read");
    assert_eq!(fresh.server_revision, base);
    second.server_revision = base;
    adapter
        .update_row(TABLE, with_pn(&second, "R-c"), "three")
        .expect("third");
}

#[test]
fn a_row_without_a_base_is_written_unchecked() {
    let (_tmp, adapter, read) = seeded();
    let mut r = with_pn(&read, "R-created");
    r.server_revision = None;
    adapter.update_row(TABLE, r, "no base").expect("unchecked");
}

#[test]
fn a_stale_delete_is_refused_and_a_current_one_succeeds() {
    let (_tmp, adapter, read) = seeded();
    adapter
        .update_row(TABLE, with_pn(&read, "R-newer"), "cell")
        .expect("write");
    let id = read.row_id.into();
    let err = adapter
        .delete_row(TABLE, id, read.server_revision, "delete")
        .expect_err("stale delete refused");
    assert!(matches!(err, LibraryError::RowChanged { .. }), "{err}");
    let fresh = adapter.read_row(TABLE, id).expect("row still there");
    adapter
        .delete_row(TABLE, id, fresh.server_revision, "delete")
        .expect("current delete");
}

/// The base covers the labelled documents (#1231): a change that only adds
/// a document still makes an older copy stale, and the document survives.
#[test]
fn a_documents_only_change_makes_an_older_copy_stale() {
    let (_tmp, adapter, read) = seeded();
    let mut with_doc = read.clone();
    with_doc.set_documents(vec![
        signex_library::DatasheetDoc::new("TI", DatasheetRef::url("https://ti.example/r.pdf")),
        signex_library::DatasheetDoc::new("Errata", DatasheetRef::url("https://ti.example/e.pdf")),
    ]);
    adapter
        .update_row(TABLE, with_doc.clone(), "documents")
        .expect("first write from the fresh base");
    let err = adapter
        .update_row(TABLE, with_pn(&read, "R-stale"), "stale")
        .expect_err("stale base must be refused");
    assert!(matches!(err, LibraryError::RowChanged { .. }), "{err}");
    let stored = adapter.read_row(TABLE, read.row_id.into()).expect("read");
    assert_eq!(stored.documents(), with_doc.documents());
}

/// A local library hands out no primitive revisions, so a save that names a
/// base is refused instead of dropping it.
#[test]
fn a_local_primitive_save_with_a_base_is_refused() {
    let (_tmp, adapter, _row) = seeded();
    let fp = signex_library::primitive::Footprint::empty("FP");
    let err = adapter
        .save_footprint_versioned(fp, Some(3), "edit")
        .expect_err("a local library has no primitive revisions");
    assert!(matches!(err, LibraryError::Backend(_)), "{err}");
}

/// A read-only mount refuses the versioned saves before touching storage.
#[test]
fn a_read_only_mount_refuses_versioned_primitive_saves() {
    use signex_library::ReadOnlyAdapter;
    use signex_library::primitive::{Footprint, SimKind, SimModel, Symbol};

    let (_tmp, adapter, _row) = seeded();
    let ro = ReadOnlyAdapter::new(Box::new(adapter));
    for err in [
        ro.save_footprint_versioned(Footprint::empty("FP"), None, "m")
            .unwrap_err(),
        ro.save_sim_versioned(SimModel::empty("SIM", SimKind::Spice3), Some(1), "m")
            .unwrap_err(),
        ro.save_symbol_versioned(Symbol::empty("S"), None, "m")
            .unwrap_err(),
    ] {
        assert!(matches!(err, LibraryError::ReadOnly { .. }), "{err}");
    }
}
