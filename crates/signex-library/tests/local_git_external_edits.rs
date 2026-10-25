//! #531 — a `.snxlib` edited by another process while it is mounted.
//!
//! `LocalGitAdapter` used to parse the file once at `open()` and serve
//! every read from that copy, so a row written by a second window, a
//! `git pull` or a text editor stayed invisible for the whole session, and
//! the next in-app mutation wrote the stale copy back over it. These tests
//! stand in for the "other process" with a second adapter on the same file.

#![cfg(feature = "local-git")]

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
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(internal_pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, Uuid::now_v7()),
        footprint_ref: None,
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
    }
}

fn pns(adapter: &LocalGitAdapter, table: &str) -> Vec<String> {
    let mut out: Vec<String> = adapter
        .read_table(table)
        .expect("read_table")
        .iter()
        .map(|r| r.internal_pn.to_string())
        .collect();
    out.sort();
    out
}

#[test]
fn row_added_by_another_process_is_visible_to_the_mounted_adapter() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();
    assert_eq!(pns(&mounted, "R"), ["R-1"]);

    let other_process = LocalGitAdapter::open(&file).unwrap();
    other_process
        .insert_row("R", row("R-2", lib), "external")
        .unwrap();

    assert_eq!(pns(&mounted, "R"), ["R-1", "R-2"]);
}

#[test]
fn table_created_by_another_process_is_listed() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();

    let other_process = LocalGitAdapter::open(&file).unwrap();
    other_process
        .insert_row("C", row("C-1", lib), "external")
        .unwrap();

    assert_eq!(mounted.list_tables().unwrap(), ["C", "R"]);
}

/// The in-app edit must be applied on top of the external one, not write a
/// stale copy over it.
#[test]
fn in_app_edit_does_not_overwrite_an_external_row() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();

    let other_process = LocalGitAdapter::open(&file).unwrap();
    other_process
        .insert_row("R", row("R-2", lib), "external")
        .unwrap();

    mounted.insert_row("R", row("R-3", lib), "in-app").unwrap();

    let reopened = LocalGitAdapter::open(&file).unwrap();
    assert_eq!(pns(&reopened, "R"), ["R-1", "R-2", "R-3"]);
}

/// A half-written or hand-broken file must neither blank the mount nor be
/// overwritten by an in-app edit.
#[test]
fn unparseable_external_edit_keeps_last_good_view_and_blocks_writes() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();

    let garbage = "this is not a snxlib <<<<<<< HEAD\n";
    std::fs::write(&file, garbage).unwrap();

    assert_eq!(pns(&mounted, "R"), ["R-1"], "reads keep the last good copy");
    let err: LibraryError = mounted
        .insert_row("R", row("R-2", lib), "in-app")
        .expect_err("a write over an unreadable file must be refused");
    assert!(
        !matches!(err, LibraryError::NotFound(_)),
        "unexpected error kind: {err:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        garbage,
        "the externally-broken file must be left exactly as found"
    );
}

/// Counts `warn!` events on the library target while a closure runs.
mod warning_capture {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tracing::span::{Attributes, Id, Record};
    use tracing::{Event, Level, Metadata, Subscriber};

    struct CountingSubscriber(Arc<AtomicUsize>);

    impl Subscriber for CountingSubscriber {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _: &Id, _: &Record<'_>) {}
        fn record_follows_from(&self, _: &Id, _: &Id) {}
        fn event(&self, event: &Event<'_>) {
            if *event.metadata().level() == Level::WARN
                && event.metadata().target() == "signex::library"
            {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        fn enter(&self, _: &Id) {}
        fn exit(&self, _: &Id) {}
    }

    /// Run `f`, returning how many library warnings it emitted.
    pub fn count_warnings(f: impl FnOnce()) -> usize {
        let count = Arc::new(AtomicUsize::new(0));
        tracing::subscriber::with_default(CountingSubscriber(count.clone()), f);
        count.load(Ordering::SeqCst)
    }
}

/// A file that vanished must warn once per distinct state, not on every
/// read — reads happen on every refresh and would flood the Messages panel.
#[test]
fn a_missing_file_warns_once_not_on_every_read() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();
    let saved = std::fs::read(&file).unwrap();
    std::fs::remove_file(&file).unwrap();

    let warned = warning_capture::count_warnings(|| {
        for _ in 0..5 {
            assert_eq!(pns(&mounted, "R"), ["R-1"], "last good copy is served");
        }
    });
    assert_eq!(warned, 1, "five reads of the same missing state");

    // The file comes back, then goes away again: a new episode warns again.
    std::fs::write(&file, &saved).unwrap();
    let _ = mounted.list_tables();
    std::fs::remove_file(&file).unwrap();
    let warned_again = warning_capture::count_warnings(|| {
        for _ in 0..3 {
            let _ = mounted.list_tables();
        }
    });
    assert_eq!(warned_again, 1, "a fresh missing episode warns once more");
}

/// An external edit that keeps both the byte length and the mtime tick
/// (HFS+ 1 s, FAT/exFAT 2 s) is invisible to the stat stamp; a mutation
/// must still not write the stale copy over it.
#[test]
fn same_length_same_mtime_external_edit_is_not_overwritten_by_a_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();

    let before = std::fs::metadata(&file).unwrap();
    let original = std::fs::read_to_string(&file).unwrap();
    assert!(original.contains("R-1"));
    let edited = original.replace("R-1", "R-9");
    assert_eq!(edited.len(), original.len());
    std::fs::write(&file, &edited).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(before.modified().unwrap())
        .unwrap();
    let after = std::fs::metadata(&file).unwrap();
    assert_eq!(after.len(), before.len());
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());

    mounted.insert_row("R", row("R-3", lib), "in-app").unwrap();

    let reopened = LocalGitAdapter::open(&file).unwrap();
    let rows = pns(&reopened, "R");
    assert!(
        rows.contains(&"R-9".to_string()),
        "external edit lost: {rows:?}"
    );
    assert!(
        rows.contains(&"R-3".to_string()),
        "in-app row missing: {rows:?}"
    );
    assert!(
        !rows.contains(&"R-1".to_string()),
        "stale row resurrected: {rows:?}"
    );
}

/// Hold the write lock of the library at `file` from this thread, as another
/// Signex process would while it writes (#832).
fn hold_write_lock(file: &Path) -> std::fs::File {
    let lock = signex_library::write_lock_path(file);
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    let handle = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock)
        .unwrap();
    handle.lock().unwrap();
    handle
}

/// #832: a mutation waits while another process is between its reload and
/// its write, and then applies on top of what that process wrote.
#[test]
fn a_mutation_waits_for_another_process_holding_the_write_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();

    let held = hold_write_lock(&file);
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop(held);
    });
    let started = std::time::Instant::now();
    mounted.insert_row("R", row("R-2", lib), "after").unwrap();
    let waited = started.elapsed();
    release.join().unwrap();

    assert!(
        waited >= std::time::Duration::from_millis(250),
        "the write must wait for the lock, took {waited:?}"
    );
    assert_eq!(pns(&mounted, "R"), ["R-1", "R-2"]);
}

/// #832: a lock held past the wait refuses the edit with a reason, and the
/// file is left exactly as the other process has it.
#[test]
fn a_mutation_is_refused_while_another_process_keeps_the_write_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();
    let before = std::fs::read(&file).unwrap();

    let _held = hold_write_lock(&file);
    let err = mounted
        .insert_row("R", row("R-2", lib), "blocked")
        .expect_err("refused while the lock is held");

    assert!(
        err.to_string().contains("another Signex process"),
        "unexpected error: {err}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), before, "file untouched");
}

/// #832: two processes inserting at the same time keep every row; neither
/// write drops the other's.
#[test]
fn concurrent_writers_on_one_library_lose_no_rows() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-0", lib), "seed").unwrap();
    drop(mounted);

    let writers: Vec<_> = ["A", "B"]
        .into_iter()
        .map(|who| {
            let file = file.clone();
            std::thread::spawn(move || {
                let adapter = LocalGitAdapter::open(&file).unwrap();
                for i in 0..10 {
                    adapter
                        .insert_row("R", row(&format!("{who}-{i}"), lib), "concurrent")
                        .unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }

    let check = LocalGitAdapter::open(&file).unwrap();
    assert_eq!(pns(&check, "R").len(), 21, "{:?}", pns(&check, "R"));
}

/// #892 review: a library edited before version control has a hidden lock
/// file next to it; enabling version control must not commit it, and the
/// lock moves into the git directory from then on.
#[test]
fn enabling_version_control_does_not_commit_the_write_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (file, mounted, lib) = mounted_library(dir.path());
    mounted.insert_row("R", row("R-1", lib), "seed").unwrap();
    let library_dir = file.parent().unwrap();
    assert!(
        std::fs::read_dir(library_dir)
            .unwrap()
            .any(|e| signex_library::is_write_lock_file(&e.unwrap().path())),
        "precondition: the no-git lock file exists"
    );

    signex_library::enable_project_version_control(library_dir, false, false, None).unwrap();

    let repo = git2::Repository::open(library_dir).unwrap();
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    let names: Vec<String> = tree
        .iter()
        .filter_map(|e| e.name().ok().map(|n| n.to_string()))
        .collect();
    assert!(
        !names.iter().any(|n| n.ends_with(".signex-writelock")),
        "the lock file was committed: {names:?}"
    );
    assert!(
        signex_library::write_lock_path(&file).starts_with(repo.path()),
        "the lock lives in the git directory once version control is on"
    );
}

/// #892 review: the same no-lost-rows guarantee with version control on,
/// where each write also commits under the lock.
#[test]
fn concurrent_writers_on_a_versioned_library_lose_no_rows() {
    let dir = tempfile::tempdir().unwrap();
    signex_library::git_identity::test_identity::install();
    let file = dir.path().join("Git").join("Git.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Git".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let seed = LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init succeeds");
    seed.insert_row("R", row("R-0", library_id), "seed")
        .unwrap();
    drop(seed);

    let writers: Vec<_> = ["A", "B"]
        .into_iter()
        .map(|who| {
            let file = file.clone();
            std::thread::spawn(move || {
                let adapter = LocalGitAdapter::open(&file).unwrap();
                for i in 0..10 {
                    adapter
                        .insert_row("R", row(&format!("{who}-{i}"), library_id), "concurrent")
                        .unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }

    let check = LocalGitAdapter::open(&file).unwrap();
    assert_eq!(pns(&check, "R").len(), 21, "{:?}", pns(&check, "R"));
}
