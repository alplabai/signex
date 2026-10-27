//! Read-only mounts: every write entry point on a library mounted via
//! `LibrarySet::mount_read_only` returns the typed
//! `LibraryError::ReadOnly` and leaves the on-disk library
//! byte-identical. Real temp dirs, real `LocalGitAdapter`, no mocks.

#![cfg(feature = "local-git")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_library::adapter::{LibraryAdapter, LibraryError};
use signex_library::adapters::library_set::LibrarySet;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::library_file::{ClassEntry, FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{Footprint, PrimitiveRef, SimKind, SimModel, Symbol};
use uuid::Uuid;

fn seeded_library(dir: &Path) -> (PathBuf, Uuid, RowId) {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Stock").join("Stock.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Stock".into(),
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
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init");
    let now = chrono::Utc::now();
    let row = ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        server_revision: None,
        internal_pn: InternalPn::new("R10K"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "R10K"),
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
    };
    let row_id = RowId::from_uuid(row.row_id);
    adapter.insert_row("resistors", row, "seed row").unwrap();
    adapter
        .save_symbol(Symbol::empty("R"), "seed symbol")
        .unwrap();
    (file, library_id, row_id)
}

/// Every file under `root` (including `.git/`), keyed by relative path.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_path_buf();
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn sample_row(row_id: RowId, library_id: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: row_id.as_uuid(),
        server_revision: None,
        internal_pn: InternalPn::new("R10K"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(library_id, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "R10K"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "9.9.9".into(),
        released: true,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    }
}

fn assert_read_only<T: std::fmt::Debug>(what: &str, got: Result<T, LibraryError>) {
    match got {
        Err(LibraryError::ReadOnly { library }) => assert_eq!(library, "Stock", "{what}"),
        other => panic!("{what}: expected LibraryError::ReadOnly, got {other:?}"),
    }
}

#[test]
fn every_write_entry_point_is_refused_and_disk_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let (file, library_id, row_id) = seeded_library(dir.path());
    let root = file.parent().unwrap().to_path_buf();
    let before = snapshot(&root);

    let mut set = LibrarySet::new();
    set.mount_read_only(Box::new(LocalGitAdapter::open(&file).unwrap()))
        .unwrap();
    assert!(set.is_read_only_path(&file));
    let a = set.get(library_id).expect("mounted");
    assert!(a.is_read_only());

    let row = sample_row(row_id, library_id);
    let class = ClassEntry {
        key: "cap".into(),
        label: "Capacitor".into(),
    };
    let sim = SimModel {
        uuid: Uuid::now_v7(),
        name: "S".into(),
        kind: SimKind::Spice3,
        body: ".SUBCKT T IN OUT\n.ENDS".into(),
        default_node_map: Default::default(),
        version: "0.0.1".into(),
        released: false,
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
    };

    assert_read_only("create_empty_table", a.create_empty_table("caps", "m"));
    assert_read_only("delete_empty_table", a.delete_empty_table("resistors", "m"));
    assert_read_only("rename_table", a.rename_table("resistors", "res", "m"));
    assert_read_only(
        "update_library_classes",
        a.update_library_classes(vec![class.clone()], "m"),
    );
    assert_read_only("add_library_class", a.add_library_class(class.clone(), "m"));
    assert_read_only("remove_library_class", a.remove_library_class("cap", "m"));
    assert_read_only(
        "rename_library_class",
        a.rename_library_class("cap", class, "m"),
    );
    assert_read_only("insert_row", a.insert_row("resistors", row.clone(), "m"));
    assert_read_only("update_row", a.update_row("resistors", row, "m"));
    assert_read_only("delete_row", a.delete_row("resistors", row_id, None, "m"));
    assert_read_only("save_symbol", a.save_symbol(Symbol::empty("X"), "m"));
    assert_read_only(
        "save_footprint",
        a.save_footprint(Footprint::empty("X"), "m"),
    );
    assert_read_only("save_sim", a.save_sim(sim, "m"));
    assert_read_only(
        "rewrite_file_locked",
        a.rewrite_file_locked(&file, "m", &mut |_| Ok(Some(b"overwritten".to_vec()))),
    );
    assert_read_only(
        "commit_external_change",
        a.commit_external_change(&file, "m"),
    );

    // Pricing writes go through the same guard: without an explicit
    // override the trait default would answer `Backend`, not `ReadOnly`.
    assert_read_only(
        "store_pricing",
        a.store_pricing(
            vec![signex_library::PricingUpdate {
                table: "resistors".into(),
                row_id: row_id.as_uuid(),
                distributor: "DigiKey".into(),
                pricing: signex_library::CachedPricing {
                    sku: "SKU-1".into(),
                    currency: "USD".into(),
                    stock: Some(1),
                    breaks: Vec::new(),
                    fetched_at: chrono::Utc::now(),
                },
            }],
            "m",
        ),
    );

    assert_eq!(
        before,
        snapshot(&root),
        "a refused write must leave every on-disk byte (including .git) identical"
    );
}

#[test]
fn reads_still_work_through_a_read_only_mount() {
    let dir = tempfile::tempdir().unwrap();
    let (file, library_id, row_id) = seeded_library(dir.path());
    let mut set = LibrarySet::new();
    set.mount_read_only(Box::new(LocalGitAdapter::open(&file).unwrap()))
        .unwrap();
    let a = set.get(library_id).unwrap();
    assert_eq!(a.list_tables().unwrap(), vec!["resistors".to_string()]);
    assert_eq!(
        a.read_row("resistors", row_id).unwrap().row_id,
        row_id.as_uuid()
    );
    assert_eq!(a.list_symbols().unwrap().len(), 1);
}

/// Positive control: the same calls succeed on an ordinary mount, so
/// the refusal above comes from the read-only mark and nothing else.
#[test]
fn an_ordinary_mount_is_writable_and_not_marked_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let (file, library_id, _) = seeded_library(dir.path());
    let mut set = LibrarySet::new();
    set.mount(Box::new(LocalGitAdapter::open(&file).unwrap()))
        .unwrap();
    assert!(!set.is_read_only_path(&file));
    let a = set.get(library_id).unwrap();
    assert!(!a.is_read_only());
    a.create_empty_table("caps", "m").unwrap();
}
