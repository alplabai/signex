//! A row edit reads the stored row once (#1939): the read decides the
//! version bump, so a failed read refuses the edit instead of writing a
//! row whose change no placed part is told about.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use signex_library::{ComponentRow, LibraryAdapter, LibraryError, Manifest, RowId};
use uuid::Uuid;

use super::{RowEdit, RowStart, WriteKind, row_edit_task};
use crate::app::Message;
use crate::library::component_preview::test_row;
use crate::library::messages::LibraryMessage;
use crate::library::state::{LibraryDisplaySettings, LibraryState, OpenLibrary};
use crate::library::test_adapters::ScriptedSymbolsAdapter;
use crate::persist::run_task::outputs;

/// How the stored row answers a read.
#[derive(Clone, Copy)]
enum Read {
    Row,
    NotFound,
    Unreadable,
}

struct RowStore {
    manifest: Manifest,
    row: ComponentRow,
    read: Read,
    reads: Arc<AtomicUsize>,
    writes: Arc<Mutex<Vec<ComponentRow>>>,
}

impl LibraryAdapter for RowStore {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(Vec::new())
    }

    fn read_row(&self, _table: &str, _row_id: RowId) -> Result<ComponentRow, LibraryError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        match self.read {
            Read::Row => Ok(self.row.clone()),
            Read::NotFound => Err(LibraryError::NotFound("row".into())),
            Read::Unreadable => Err(LibraryError::Backend("table file unreadable".into())),
        }
    }

    fn update_row(
        &self,
        _table: &str,
        row: ComponentRow,
        _msg: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.writes.lock().expect("writes lock").push(row);
        Ok(Some(7))
    }
}

struct Fixture {
    state: LibraryState,
    root: PathBuf,
    row: ComponentRow,
    reads: Arc<AtomicUsize>,
    writes: Arc<Mutex<Vec<ComponentRow>>>,
}

fn fixture(read: Read) -> Fixture {
    let library_id = Uuid::now_v7();
    let root = PathBuf::from("/tmp/read-failure/Lib.snxlib");
    let row = test_row();
    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut state = LibraryState::default();
    state
        .set
        .mount(Box::new(RowStore {
            manifest: ScriptedSymbolsAdapter::new(library_id).0.manifest().clone(),
            row: row.clone(),
            read,
            reads: reads.clone(),
            writes: writes.clone(),
        }))
        .expect("mount");
    state.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Lib".into(),
        library_id,
        tables: HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    Fixture {
        state,
        root,
        row,
        reads,
        writes,
    }
}

/// Run an edit of the row's Manufacturer; the failure reason when refused.
fn edit_manufacturer(
    f: &mut Fixture,
    start: impl FnOnce(ComponentRow) -> RowStart,
) -> Result<(), String> {
    let mut changed = f.row.clone();
    changed.primary_mpn.manufacturer = "Other Corp".into();
    let changed_row = changed.clone();
    let edit = RowEdit {
        table: "R".into(),
        row_id: RowId::from_uuid(f.row.row_id),
        start: start(changed),
        patch: Some(Box::new(|row| {
            row.primary_mpn.manufacturer = "Other Corp".into()
        })),
        assets: Vec::new(),
        message: "edit",
    };
    let task = row_edit_task(
        &mut f.state,
        &f.root,
        WriteKind::CellSaved {
            table: "R".into(),
            row_id: RowId::from_uuid(f.row.row_id),
            column: "Manufacturer".into(),
            typed: "Other Corp".into(),
            saved: Box::new(changed_row),
        },
        edit,
    );
    let done = outputs(task)
        .into_iter()
        .find_map(|m| match m {
            Message::Library(LibraryMessage::WriteFinished(done)) => Some(done),
            _ => None,
        })
        .expect("the write finishes");
    done.outcome.map(|_| ()).map_err(|failure| failure.reason)
}

fn full(row: ComponentRow) -> RowStart {
    RowStart::Full(Box::new(row))
}

#[test]
fn an_unreadable_row_refuses_the_edit() {
    let mut f = fixture(Read::Unreadable);
    let outcome = edit_manufacturer(&mut f, full);

    let reason = outcome.expect_err("the edit is refused");
    assert!(reason.contains("table file unreadable"), "{reason}");
    assert!(
        reason.contains("edit refused") && reason.contains("Manufacturer"),
        "the reason says why the edit was refused: {reason}"
    );
    assert!(
        f.writes.lock().unwrap().is_empty(),
        "nothing is written without the version bump"
    );
}

/// Guard, not a regression test: it passes without the #1939 fix too. It
/// keeps the refusal from growing to cover a plain first write.
#[test]
fn a_row_that_is_not_stored_yet_is_a_plain_first_write() {
    let mut f = fixture(Read::NotFound);
    let outcome = edit_manufacturer(&mut f, full);

    assert_eq!(outcome, Ok(()));
    assert_eq!(f.writes.lock().unwrap().len(), 1);
}

/// Guard, not a regression test: the version bump worked before #1939 when
/// the read succeeded; this pins it beside the refusal that now covers the
/// failing read.
#[test]
fn a_changed_manufacturer_moves_the_row_version_on_write() {
    let mut f = fixture(Read::Row);
    let before = f.row.version.clone();
    let outcome = edit_manufacturer(&mut f, full);

    assert_eq!(outcome, Ok(()));
    let written = f.writes.lock().unwrap();
    assert_ne!(written[0].version, before, "placed parts see the change");
}

#[test]
fn an_edit_from_the_stored_row_reads_it_once() {
    let mut f = fixture(Read::Row);
    let outcome = edit_manufacturer(&mut f, |_| RowStart::FromStored {
        base_revision: Some(1),
    });

    assert_eq!(outcome, Ok(()));
    assert_eq!(f.reads.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.writes.lock().unwrap()[0].primary_mpn.manufacturer,
        "Other Corp"
    );
}
