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

// ── #1436: what the one cascade commit says, and what a failure leaves ──

use signex_library::manifest::WorkflowMode;
use signex_library::{CascadeKind, PrimitiveSave, cascade_after_saves};

fn save_of(lib: Uuid, symbol: Uuid) -> PrimitiveSave {
    PrimitiveSave {
        kind: CascadeKind::Symbol,
        library_id: lib,
        uuid: symbol,
        version: "0.0.2".into(),
    }
}

/// `n` rows bound to one symbol at `0.0.1`, inserted.
fn seed_bound(adapter: &LocalGitAdapter, lib: Uuid, symbol: Uuid, n: usize) -> Vec<ComponentRow> {
    let rows: Vec<ComponentRow> = (0..n)
        .map(|i| row(&format!("R-{i}"), lib, symbol, "0.0.1"))
        .collect();
    for r in &rows {
        adapter.insert_row(TABLE, r.clone(), "seed").unwrap();
    }
    rows
}

#[test]
fn the_cascade_commit_names_the_rows_it_moved() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let root = dir.path().join("Lib");
    let symbol = Uuid::now_v7();
    seed_bound(&adapter, lib, symbol, 3);
    let before = snxlib_commits(&root).len();

    let report =
        cascade_after_saves(&adapter, &[save_of(lib, symbol)], WorkflowMode::Personal).unwrap();
    assert_eq!(report.auto_bumped.len(), 3);

    let commits = snxlib_commits(&root);
    assert_eq!(commits.len() - before, 1, "one commit: {commits:?}");
    let msg = &commits[0];
    for pn in ["R-0", "R-1", "R-2"] {
        assert!(msg.contains(pn), "{pn} missing from {msg:?}");
    }
    assert!(!msg.contains("more"), "{msg:?}");
}

#[test]
fn a_long_row_list_in_the_commit_message_is_cut_with_a_count() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let root = dir.path().join("Lib");
    let symbol = Uuid::now_v7();
    seed_bound(&adapter, lib, symbol, 12);

    cascade_after_saves(&adapter, &[save_of(lib, symbol)], WorkflowMode::Personal).unwrap();

    let msg = snxlib_commits(&root).remove(0);
    let named = (0..12)
        .filter(|i| msg.contains(&format!("R-{i},")) || msg.contains(&format!("R-{i} ")))
        .count();
    assert!(msg.contains("+7 more"), "{msg:?}");
    assert!(msg.lines().next().unwrap().len() < 200, "{msg:?}");
    assert!(named <= 5, "{named} rows named in {msg:?}");
}

#[test]
fn released_rows_under_team_stay_put_and_the_rest_land_in_one_commit() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let root = dir.path().join("Lib");
    let symbol = Uuid::now_v7();
    let open = seed_bound(&adapter, lib, symbol, 3);
    let mut released: Vec<ComponentRow> = (0..2)
        .map(|i| row(&format!("REL-{i}"), lib, symbol, "0.0.1"))
        .collect();
    for r in &mut released {
        r.released = true;
        adapter.insert_row(TABLE, r.clone(), "seed").unwrap();
    }
    let before = snxlib_commits(&root).len();

    let report =
        cascade_after_saves(&adapter, &[save_of(lib, symbol)], WorkflowMode::Team).unwrap();

    assert_eq!(report.auto_bumped.len(), 3);
    assert_eq!(
        report.stale.len(),
        2,
        "released rows are reported, not skipped"
    );
    assert!(report.failed.is_empty());
    assert_eq!(snxlib_commits(&root).len() - before, 1);
    for r in &open {
        let got = adapter.read_row(TABLE, RowId::from_uuid(r.row_id)).unwrap();
        assert_eq!(got.symbol_version, "0.0.2");
    }
    for r in &released {
        let got = adapter.read_row(TABLE, RowId::from_uuid(r.row_id)).unwrap();
        assert_eq!(got.symbol_version, "0.0.1");
        assert_eq!(got.version, "0.0.1");
    }
}

/// A row that vanished between the scan and the write is that row's error;
/// the others still land, in the one commit.
#[test]
fn a_row_gone_at_write_time_fails_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let root = dir.path().join("Lib");
    let symbol = Uuid::now_v7();
    let rows = seed_bound(&adapter, lib, symbol, 4);
    let gone = RowId::from_uuid(rows[1].row_id);
    let targets: Vec<(String, RowId)> = rows
        .iter()
        .map(|r| (TABLE.to_string(), RowId::from_uuid(r.row_id)))
        .collect();
    adapter
        .delete_row(TABLE, gone, None, "removed meanwhile")
        .unwrap();
    let before = snxlib_commits(&root).len();

    let outcomes = adapter
        .update_rows(
            &targets,
            &mut |r| {
                r.symbol_version = "0.0.2".into();
                true
            },
            &|changed| format!("changed {}", changed.len()),
        )
        .unwrap();

    assert_eq!(outcomes.len(), 4);
    for (id, outcome) in &outcomes {
        if *id == gone {
            assert!(outcome.is_err(), "the missing row must be reported");
        } else {
            assert!(matches!(outcome, Ok(true)));
        }
    }
    let commits = snxlib_commits(&root);
    assert_eq!(commits.len() - before, 1);
    assert_eq!(
        commits[0].trim(),
        "changed 3",
        "the message counts changed rows only"
    );
}

/// The atomic write cannot create its temp file: no row moves, the file and
/// git are as they were, and every row is reported failed.
#[cfg(unix)]
#[test]
fn a_write_failure_leaves_no_partial_cascade() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let root = dir.path().join("Lib");
    let symbol = Uuid::now_v7();
    let rows = seed_bound(&adapter, lib, symbol, 4);
    let file = root.join("Lib.snxlib");
    let bytes = std::fs::read(&file).unwrap();
    let head = git2::Repository::open(&root)
        .unwrap()
        .head()
        .unwrap()
        .target();

    let writable = std::fs::metadata(&root).unwrap().permissions();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555)).unwrap();
    let probe = root.join("probe");
    if std::fs::File::create(&probe).is_ok() {
        std::fs::remove_file(&probe).unwrap();
        std::fs::set_permissions(&root, writable).unwrap();
        eprintln!("skipped: the library folder is still writable (running as root?)");
        return;
    }
    let report = cascade_after_saves(&adapter, &[save_of(lib, symbol)], WorkflowMode::Personal);
    std::fs::set_permissions(&root, writable).unwrap();

    let report = report.unwrap();
    assert!(report.auto_bumped.is_empty());
    assert_eq!(report.failed.len(), 4, "every row is reported");
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert_eq!(
        git2::Repository::open(&root)
            .unwrap()
            .head()
            .unwrap()
            .target(),
        head
    );
    for r in &rows {
        let got = adapter.read_row(TABLE, RowId::from_uuid(r.row_id)).unwrap();
        assert_eq!(got.symbol_version, "0.0.1", "memory matches disk");
    }
    // The next pass retries all of them.
    let again =
        cascade_after_saves(&adapter, &[save_of(lib, symbol)], WorkflowMode::Personal).unwrap();
    assert_eq!(again.auto_bumped.len(), 4);
}

/// A malformed stored row makes the scan fail with an error a retry cannot
/// fix; an IO error can be retried.
#[test]
fn a_malformed_row_fails_the_scan_with_a_permanent_error() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = init(&dir);
    let symbol = Uuid::now_v7();
    seed_bound(&adapter, lib, symbol, 2);
    let file = dir.path().join("Lib").join("Lib.snxlib");
    let mut lf =
        signex_library::LibraryFile::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
    std::sync::Arc::make_mut(&mut lf.tables.get_mut(TABLE).unwrap().rows[0])
        .cells
        .insert("row_id".into(), "not-a-uuid".into());
    std::fs::write(&file, lf.write().unwrap()).unwrap();

    let err =
        cascade_after_saves(&adapter, &[save_of(lib, symbol)], WorkflowMode::Personal).unwrap_err();
    assert!(signex_library::retry_cannot_fix(&err), "{err}");
    assert!(!signex_library::retry_cannot_fix(
        &signex_library::adapter::LibraryError::Io(std::io::Error::other("disk full"))
    ));
}

/// Opening a library whose format token this build does not read is a
/// permanent refusal.
#[test]
fn an_unsupported_format_token_is_a_permanent_open_error() {
    let dir = tempfile::tempdir().unwrap();
    let (_adapter, _lib) = init(&dir);
    let file = dir.path().join("Lib").join("Lib.snxlib");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replacen("snxlib/2", "snxlib/99", 1)).unwrap();
    let Err(err) = LocalGitAdapter::open(&file) else {
        panic!("a library in an unknown format must be refused");
    };
    assert!(signex_library::retry_cannot_fix(&err), "{err}");
}
