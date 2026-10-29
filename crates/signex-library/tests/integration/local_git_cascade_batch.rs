//! A symbol save moves every row bound to it in one `.snxlib` write and one
//! git commit (#1362), not one of each per row.

use std::path::Path;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{PrimitiveRef, Symbol, SymbolPin};
use uuid::Uuid;

const TABLE: &str = "R";

fn init(dir: &tempfile::TempDir) -> (LocalGitAdapter, Uuid) {
    signex_library::git_identity::test_identity::install();
    let file = dir.path().join("Lib").join("Lib.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Lib".into(),
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
    .unwrap();
    (adapter, library_id)
}

fn row(pn: &str, library_id: Uuid, symbol: Uuid, pinned: &str) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, symbol),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: pinned.into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn symbol(version: &str) -> Symbol {
    let mut s = Symbol::empty("R-SYM");
    s.pins.clear();
    s.pins.push(SymbolPin::new("1", "A"));
    s.version = version.into();
    s
}

/// Commits whose tree holds a different `Lib.snxlib` than their parent's:
/// the number of times the library file was committed.
fn snxlib_commits(root: &Path) -> Vec<String> {
    let repo = git2::Repository::open(root).unwrap();
    let blob_of = |c: &git2::Commit| {
        c.tree()
            .unwrap()
            .get_path(Path::new("Lib.snxlib"))
            .ok()
            .map(|e| e.id())
    };
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    let mut out = Vec::new();
    for oid in walk {
        let commit = repo.find_commit(oid.unwrap()).unwrap();
        let parent = commit.parents().next();
        if blob_of(&commit) != parent.as_ref().and_then(blob_of) {
            out.push(commit.message().unwrap().to_string());
        }
    }
    out
}

fn total_commits(root: &Path) -> usize {
    let repo = git2::Repository::open(root).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    walk.count()
}

#[test]
fn a_symbol_bound_by_many_rows_is_one_snxlib_commit_and_all_rows_bump() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let root = dir.path().join("Lib");
    let sym = symbol("0.0.1");
    let sym_uuid = sym.uuid;
    adapter.save_symbol(sym.clone(), "add symbol").unwrap();

    let bound: Vec<ComponentRow> = (0..5)
        .map(|i| row(&format!("R-{i}"), lib, sym_uuid, "0.0.1"))
        .collect();
    for r in &bound {
        adapter.insert_row(TABLE, r.clone(), "seed").unwrap();
    }
    // Already at the version the save is about to produce: untouched.
    let current = row("R-CURRENT", lib, sym_uuid, "0.0.2");
    adapter.insert_row(TABLE, current.clone(), "seed").unwrap();

    let (snx_before, all_before) = (snxlib_commits(&root).len(), total_commits(&root));
    let mut next = sym;
    next.version = "0.0.2".into();
    adapter.save_symbol(next, "edit symbol").unwrap();

    let snx_after = snxlib_commits(&root);
    assert_eq!(
        snx_after.len() - snx_before,
        1,
        "one .snxlib commit for the whole cascade: {snx_after:?}"
    );
    assert!(
        snx_after[0].starts_with("cascade: bump symbol"),
        "{snx_after:?}"
    );
    assert!(
        total_commits(&root) - all_before <= 2,
        "the symbol file's commit plus one cascade commit"
    );

    for r in &bound {
        let got = adapter.read_row(TABLE, RowId::from_uuid(r.row_id)).unwrap();
        assert_eq!(got.symbol_version, "0.0.2");
        assert_eq!(got.version, "0.0.2", "own version patch-bumped");
    }
    let untouched = adapter
        .read_row(TABLE, RowId::from_uuid(current.row_id))
        .unwrap();
    assert_eq!(
        untouched.version, "0.0.1",
        "a row at the version is left alone"
    );
    assert_eq!(untouched.updated, current.updated);
}

#[test]
fn a_cascade_with_nothing_to_bump_commits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let root = dir.path().join("Lib");
    let sym = symbol("0.0.2");
    adapter.save_symbol(sym.clone(), "add symbol").unwrap();
    adapter
        .insert_row(TABLE, row("R-1", lib, sym.uuid, "0.0.2"), "seed")
        .unwrap();
    let before = total_commits(&root);
    adapter.save_symbol(sym, "same version").unwrap();
    assert!(total_commits(&root) - before <= 1, "no cascade commit");
}
