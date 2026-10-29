//! #416: the cost of a one-row edit on a large table, and that the faster
//! write still produces the same bytes and refuses a stale edit.
//!
//! The timing tests are ignored by default. Run them with
//! `cargo test -p signex-library --test integration --release -- table_write_cost --ignored --nocapture`.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Timed repetitions per operation; the fastest is reported.
const RUNS: usize = 5;

use chrono::Utc;
use signex_library::adapter::{LibraryAdapter, LibraryError};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use signex_library::distributor::PriceBreak;
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::DistributorListing;
use signex_library::pricing::{CachedPricing, PricingUpdate};
use signex_library::*;
use uuid::Uuid;

fn price(row_id: Uuid) -> PricingUpdate {
    PricingUpdate {
        table: "resistors".into(),
        row_id,
        distributor: "DigiKey".into(),
        pricing: CachedPricing {
            sku: "DK-1".into(),
            currency: "USD".into(),
            stock: Some(100),
            breaks: vec![
                PriceBreak {
                    qty: 1,
                    unit_price: 0.1,
                },
                PriceBreak {
                    qty: 100,
                    unit_price: 0.05,
                },
            ],
            fetched_at: Utc::now(),
        },
    }
}

fn row(n: usize, lib: Uuid) -> ComponentRow {
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(format!("R0805_{n}")),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::now_v7())),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", format!("RC0805FR-{n}")),
        alternates: Vec::new(),
        supply: vec![DistributorListing::new("DigiKey", "DK-1")],
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
    }
}

/// A git-backed library at `<dir>/lib/lib.snxlib` whose `resistors` table
/// holds `size` rows, the first of which is returned as read back (with its
/// base). The bulk rows are written straight to the file and left
/// uncommitted: one insert per row would be quadratic.
fn seeded_library(dir: &Path, size: usize) -> (PathBuf, LocalGitAdapter, ComponentRow, Uuid) {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("lib").join("lib.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let lib = manifest.library_id;
    let opts = LibraryInitOptions {
        enable_git: true,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(&file, manifest, opts).unwrap();
    let seed = row(0, lib);
    adapter
        .insert_row("resistors", seed.clone(), "seed")
        .unwrap();
    let mut lf = LibraryFile::parse(&fs::read_to_string(&file).unwrap()).unwrap();
    let table = lf.tables.get_mut("resistors").unwrap();
    let stored = table.rows[0].clone();
    for n in 1..size {
        let mut r = stored.clone();
        let cells = &mut std::sync::Arc::make_mut(&mut r).cells;
        cells.insert("row_id".into(), Uuid::now_v7().to_string());
        // Part numbers are unique library-wide (#1363).
        cells.insert("internal_pn".into(), format!("R0805_{n}"));
        table.rows.push(r);
    }
    fs::write(&file, lf.write().unwrap()).unwrap();
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let seed = adapter
        .read_row("resistors", RowId::from_uuid(seed.row_id))
        .unwrap();
    (file, adapter, seed, lib)
}

/// Blob id of `rel` in HEAD's tree and in the index.
fn committed_and_staged(root: &Path, rel: &str) -> (git2::Oid, git2::Oid) {
    let repo = git2::Repository::open(root).unwrap();
    let head = repo.head().unwrap().peel_to_tree().unwrap();
    let committed = head.get_path(Path::new(rel)).unwrap().id();
    let staged = repo
        .index()
        .unwrap()
        .get_path(Path::new(rel), 0)
        .unwrap()
        .id;
    (committed, staged)
}

/// An edit writes exactly what encoding the whole edited library gives, the
/// other rows are untouched, and the commit and the index both hold the
/// bytes on disk (hashed once now, #416).
#[test]
fn a_row_edit_writes_the_whole_file_encoding_and_commits_those_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (file, adapter, seed, _) = seeded_library(dir.path(), 300);
    let before = LibraryFile::parse(&fs::read_to_string(&file).unwrap()).unwrap();

    let mut edited = seed.clone();
    edited.version = "0.0.2".into();
    adapter.update_row("resistors", edited, "edit").unwrap();

    let on_disk = fs::read_to_string(&file).unwrap();
    let after = LibraryFile::parse(&on_disk).unwrap();
    assert_eq!(
        after.write().unwrap(),
        on_disk,
        "not the canonical encoding"
    );
    let seed_id = seed.row_id.to_string();
    let others = |lf: &LibraryFile| {
        let mut rows: Vec<_> = lf.tables["resistors"]
            .rows
            .iter()
            .filter(|r| r.cells["row_id"] != seed_id)
            .map(|r| r.cells.clone())
            .collect();
        rows.sort_by(|a, b| a["row_id"].cmp(&b["row_id"]));
        rows
    };
    assert_eq!(others(&after), others(&before));
    assert_eq!(after.tables["resistors"].rows.len(), 300);

    let on_disk_blob = git2::Oid::hash_file(git2::ObjectType::Blob, &file).unwrap();
    let (committed, staged) = committed_and_staged(dir.path().join("lib").as_path(), "lib.snxlib");
    assert_eq!(committed, on_disk_blob);
    assert_eq!(staged, on_disk_blob);
}

/// A write from a stale base is refused before anything is written or
/// committed (#1108).
#[test]
fn a_stale_edit_on_a_large_library_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (file, adapter, seed, _) = seeded_library(dir.path(), 300);
    let mut first = seed.clone();
    first.version = "0.0.2".into();
    adapter.update_row("resistors", first, "first").unwrap();
    let bytes = fs::read(&file).unwrap();
    let root = dir.path().join("lib");
    let head = committed_and_staged(&root, "lib.snxlib");

    let mut stale = seed.clone();
    stale.version = "0.0.3".into();
    let err = adapter.update_row("resistors", stale, "stale").unwrap_err();
    assert!(matches!(err, LibraryError::RowChanged { .. }), "{err}");
    let err = adapter
        .delete_row(
            "resistors",
            RowId::from_uuid(seed.row_id),
            seed.server_revision,
            "stale delete",
        )
        .unwrap_err();
    assert!(matches!(err, LibraryError::RowChanged { .. }), "{err}");
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(committed_and_staged(&root, "lib.snxlib"), head);
}

/// The path the app actually takes: `LocalGitAdapter` edits rewrite the whole
/// `.snxlib` (clone, encode, read-back verify, atomic write, commit).
#[test]
#[ignore = "timing measurement for #416"]
fn adapter_one_row_edit_cost_by_library_size() {
    for size in [1_000usize, 10_000, 50_000] {
        let dir = tempfile::tempdir().unwrap();
        let (file, adapter, seed, lib) = seeded_library(dir.path(), size);
        // Every tenth row carries a cached price.
        let prices: Vec<PricingUpdate> = adapter
            .read_table("resistors")
            .unwrap()
            .iter()
            .step_by(10)
            .map(|r| price(r.row_id))
            .collect();
        adapter
            .store_pricing(prices, &std::sync::atomic::AtomicBool::new(false), "prices")
            .unwrap();
        let bytes = fs::metadata(&file).unwrap().len();
        // The read + hash every edit starts with, to spot an external change.
        let mut read_hash = Duration::MAX;
        for _ in 0..RUNS {
            let t = Instant::now();
            let text = fs::read_to_string(&file).unwrap();
            let mut hasher = DefaultHasher::new();
            text.hash(&mut hasher);
            std::hint::black_box(hasher.finish());
            read_hash = read_hash.min(t.elapsed());
        }

        // Best of RUNS: one shot is mostly scheduler noise on a busy machine.
        let (mut upd, mut ins, mut del) = (Duration::MAX, Duration::MAX, Duration::MAX);
        let mut base = seed.server_revision;
        for n in 0..RUNS {
            let mut edited = seed.clone();
            edited.version = format!("0.0.{}", n + 2);
            edited.server_revision = base;
            let t = Instant::now();
            base = adapter.update_row("resistors", edited, "edit").unwrap();
            upd = upd.min(t.elapsed());
            let added = row(size + n, lib);
            let t = Instant::now();
            adapter
                .insert_row("resistors", added.clone(), "add")
                .unwrap();
            ins = ins.min(t.elapsed());
            let t = Instant::now();
            adapter
                .delete_row("resistors", RowId::from_uuid(added.row_id), None, "del")
                .unwrap();
            del = del.min(t.elapsed());
        }
        println!(
            "adapter rows={size} bytes={bytes} update_row={upd:?} insert_row={ins:?} delete_row={del:?} read_hash={read_hash:?}"
        );
    }
}

/// The read side of the same library: one row by id, and the whole table,
/// cold (first call after open) and warm (best of RUNS).
#[test]
#[ignore = "timing measurement for #1437"]
fn adapter_read_cost_by_library_size() {
    for size in [1_000usize, 10_000, 50_000] {
        let dir = tempfile::tempdir().unwrap();
        let (_file, adapter, seed, _) = seeded_library(dir.path(), size);
        let id = RowId::from_uuid(seed.row_id);
        let t = Instant::now();
        std::hint::black_box(adapter.read_table("resistors").unwrap());
        let table_cold = t.elapsed();
        let (mut row, mut table) = (Duration::MAX, Duration::MAX);
        for _ in 0..RUNS {
            let t = Instant::now();
            std::hint::black_box(adapter.read_row("resistors", id).unwrap());
            row = row.min(t.elapsed());
            let t = Instant::now();
            std::hint::black_box(adapter.read_table("resistors").unwrap());
            table = table.min(t.elapsed());
        }
        println!(
            "adapter rows={size} read_row={row:?} read_table_cold={table_cold:?} read_table_warm={table:?}"
        );
    }
}

/// Total size in bytes of every file under `dir`.
fn dir_bytes(dir: &Path) -> u64 {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// `.git/objects` growth per one-row edit: each edit commits a whole new
/// `.snxlib` blob, so growth tracks the file size, not the edit.
#[test]
#[ignore = "measurement for #1438"]
fn git_objects_growth_per_edit_by_library_size() {
    const EDITS: usize = 10;
    for size in [1_000usize, 10_000, 50_000] {
        let dir = tempfile::tempdir().unwrap();
        let (file, adapter, seed, _) = seeded_library(dir.path(), size);
        let objects = dir.path().join("lib").join(".git").join("objects");
        let bytes = fs::metadata(&file).unwrap().len();
        let mut base = seed.server_revision;
        // The first edit also commits the bulk-seeded rows; leave it out.
        let mut edited = seed.clone();
        edited.version = "0.0.2".into();
        edited.server_revision = base;
        base = adapter.update_row("resistors", edited, "warm").unwrap();
        let before = dir_bytes(&objects);
        for n in 0..EDITS {
            let mut edited = seed.clone();
            edited.version = format!("0.0.{}", n + 3);
            edited.server_revision = base;
            base = adapter.update_row("resistors", edited, "edit").unwrap();
        }
        let grown = dir_bytes(&objects) - before;
        println!(
            "adapter rows={size} snxlib_bytes={bytes} edits={EDITS} objects_growth={grown} per_edit={}",
            grown / EDITS as u64
        );
        // What a repack would leave: the command-line `git gc` deltas the
        // near-identical blobs. Skipped when `git` is not installed.
        let root = dir.path().join("lib");
        let gc = std::process::Command::new("git")
            .args(["gc", "--quiet"])
            .current_dir(&root)
            .status();
        if gc.is_ok_and(|s| s.success()) {
            let packed = dir_bytes(&objects);
            println!(
                "adapter rows={size} after_gc_objects_bytes={packed} (was {})",
                before + grown
            );
        }
    }
}

/// A project repo at a fresh temp dir holding `file.txt` (committed).
fn project_repo() -> (tempfile::TempDir, LocalGitProjectAdapter) {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file.txt"), "one\n").unwrap();
    let project = LocalGitProjectAdapter::open_or_init(dir.path().to_path_buf()).unwrap();
    project.commit_path(Path::new("file.txt"), "first").unwrap();
    (dir, project)
}

/// The index is re-read on every write attempt: a path another process
/// staged while our save waited on its `index.lock` stays staged.
#[test]
fn a_save_waiting_on_the_index_lock_keeps_what_the_holder_staged() {
    let (dir, project) = project_repo();
    let git = dir.path().join(".git");
    fs::write(dir.path().join("other.txt"), "other\n").unwrap();
    // The index the lock holder will publish: ours plus `other.txt`.
    let original = fs::read(git.join("index")).unwrap();
    let repo = git2::Repository::open(dir.path()).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("other.txt")).unwrap();
    index.write().unwrap();
    let published = fs::read(git.join("index")).unwrap();
    fs::write(git.join("index"), &original).unwrap();

    fs::write(git.join("index.lock"), b"").unwrap();
    fs::write(dir.path().join("file.txt"), "two\n").unwrap();
    std::thread::scope(|s| {
        let first = git2::Repository::open(dir.path())
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        let save = s.spawn(|| project.commit_path(Path::new("file.txt"), "second"));
        // Wait for the save to commit (HEAD moves); it then blocks on the
        // lock. A fixed sleep raced slow CI runners (#1553).
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            let head = git2::Repository::open(dir.path())
                .ok()
                .and_then(|r| r.head().ok().and_then(|h| h.target()));
            if head.is_some_and(|h| h != first) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the save never committed"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        fs::write(git.join("index.new"), &published).unwrap();
        fs::rename(git.join("index.new"), git.join("index")).unwrap();
        fs::remove_file(git.join("index.lock")).unwrap();
        save.join().unwrap().unwrap().expect("a commit was made");
    });

    let repo = git2::Repository::open(dir.path()).unwrap();
    let index = repo.index().unwrap();
    assert!(
        index.get_path(Path::new("other.txt"), 0).is_some(),
        "the lock holder's staged path was dropped"
    );
    let (committed, staged) = committed_and_staged(dir.path(), "file.txt");
    assert_eq!(committed, staged);
}

/// With an eol filter, the committed blob is the filtered content (what
/// `git add` stores), not a hash of the raw bytes on disk.
#[test]
fn a_crlf_file_under_an_eol_attribute_commits_its_filtered_blob() {
    let (dir, project) = project_repo();
    fs::write(dir.path().join(".gitattributes"), "* text=auto eol=crlf\n").unwrap();
    let path = dir.path().join("file.txt");
    fs::write(&path, "one\r\ntwo\r\n").unwrap();
    project.commit_path(Path::new("file.txt"), "crlf").unwrap();

    let repo = git2::Repository::open(dir.path()).unwrap();
    let filtered = repo.blob_path(&path).unwrap();
    let raw = git2::Oid::hash_file(git2::ObjectType::Blob, &path).unwrap();
    assert_ne!(filtered, raw, "the filter did not apply");
    let (committed, staged) = committed_and_staged(dir.path(), "file.txt");
    assert_eq!(committed, filtered);
    assert_eq!(staged, filtered);
    assert_eq!(
        repo.find_blob(committed).unwrap().content(),
        b"one\ntwo\n".as_slice()
    );
}
