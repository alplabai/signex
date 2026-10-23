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
