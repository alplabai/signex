//! #859 — the Component Preview editor's reads (a tab's first visit, a row
//! reload) run on a worker; the messages that land them finish a close that
//! was waiting behind them.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use signex_library::{
    ComponentRow, LibraryAdapter, LibraryError, Manifest, PrimitiveRef, RowId, Symbol,
};
use uuid::Uuid;

use crate::app::path_ops::settle_task;
use crate::app::{Message, Signex};
use crate::library::component_preview::test_row;
use crate::library::messages::EditorMsg;
use crate::library::state::{
    ComponentPreviewState, EditorAddress, LibraryDisplaySettings, OpenLibrary, PreviewTab,
};
use crate::library::test_adapters::ScriptedSymbolsAdapter;
use crate::persist::run_task::outputs;

struct Shelf {
    manifest: Manifest,
    row: ComponentRow,
    reads: Arc<AtomicUsize>,
}

impl LibraryAdapter for Shelf {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(Vec::new())
    }

    fn read_row(&self, _table: &str, _row_id: RowId) -> Result<ComponentRow, LibraryError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.row.clone())
    }

    fn get_symbol(&self, _uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(Symbol::empty("Disk"))
    }
}

struct Fixture {
    app: Signex,
    root: PathBuf,
    address: EditorAddress,
    reads: Arc<AtomicUsize>,
}

/// A mounted library storing a row "Stored", and an open preview of a copy
/// of it ("Old") that has drawn nothing yet.
fn fixture() -> Fixture {
    let library_id = Uuid::now_v7();
    let root = PathBuf::from("/tmp/editor-reads/Lib.snxlib");
    let mut stored = test_row();
    stored.symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
    stored.description = "Stored".into();
    let mut copy = stored.clone();
    copy.description = "Old".into();
    let address = EditorAddress::new(root.clone(), "R".into(), RowId::from_uuid(stored.row_id));
    let reads = Arc::new(AtomicUsize::new(0));
    let manifest = ScriptedSymbolsAdapter::new(library_id).0.manifest().clone();
    let (mut app, _task) = Signex::new();
    app.library.open_libraries.clear();
    app.library
        .set
        .mount(Box::new(Shelf {
            manifest,
            row: stored,
            reads: reads.clone(),
        }))
        .expect("mount");
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Lib".into(),
        library_id,
        tables: std::collections::HashMap::new(),
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
    app.library.editors.insert(
        address.clone(),
        ComponentPreviewState::from_row(root.clone(), "R".into(), copy),
    );
    Fixture {
        app,
        root,
        address,
        reads,
    }
}

impl Fixture {
    fn event(&mut self, msg: EditorMsg) -> iced::Task<Message> {
        self.app.handle_editor_event(self.address.clone(), msg)
    }

    fn preview(&self) -> &ComponentPreviewState {
        &self.app.library.editors[&self.address]
    }
}

#[test]
fn the_first_visit_of_a_tab_reads_nothing_in_update() {
    let mut f = fixture();

    let task = f.event(EditorMsg::SelectTab(PreviewTab::Preview));

    assert_eq!(f.reads.load(Ordering::SeqCst), 0, "update only plans");
    assert!(f.preview().symbol.is_none());
    settle_task(&mut f.app, task);
    assert_eq!(
        f.preview().symbol.as_ref().map(|s| s.name.as_str()),
        Some("Disk")
    );
}

#[test]
fn reloading_a_row_reads_nothing_in_update() {
    let mut f = fixture();

    let task = f.event(EditorMsg::ReloadRequested);

    assert_eq!(f.reads.load(Ordering::SeqCst), 0, "update only plans");
    assert_eq!(f.preview().row.description, "Old");
    settle_task(&mut f.app, task);
    assert_eq!(f.preview().row.description, "Stored");
}

#[test]
fn a_reload_asked_of_a_clean_copy_keeps_what_is_typed_before_it_lands() {
    let mut f = fixture();

    let task = f.event(EditorMsg::ReloadRequested);
    // The read is still queued; the user types.
    let preview = f.app.library.editors.get_mut(&f.address).expect("preview");
    preview.row.description = "Typed".into();
    preview.dirty = true;
    settle_task(&mut f.app, task);

    assert_eq!(f.preview().row.description, "Typed");
    assert!(f.preview().dirty);
}

#[test]
fn a_confirmed_reload_keeps_what_is_typed_after_the_confirm() {
    let mut f = fixture();
    let preview = f.app.library.editors.get_mut(&f.address).expect("preview");
    preview.row.description = "Confirmed over".into();
    preview.dirty = true;

    let task = f.event(EditorMsg::ReloadConfirmed);
    let preview = f.app.library.editors.get_mut(&f.address).expect("preview");
    preview.row.description = "Typed after".into();
    settle_task(&mut f.app, task);

    assert_eq!(f.preview().row.description, "Typed after");
}

#[test]
fn a_confirmed_reload_replaces_the_edits_that_existed_at_the_confirm() {
    let mut f = fixture();
    let preview = f.app.library.editors.get_mut(&f.address).expect("preview");
    preview.row.description = "Confirmed over".into();
    preview.dirty = true;

    let task = f.event(EditorMsg::ReloadConfirmed);
    settle_task(&mut f.app, task);

    assert_eq!(f.preview().row.description, "Stored");
}

/// A close asked while the read runs waits behind it; the message that
/// lands the read must finish it.
fn assert_close_finishes_when_the_read_lands(mut f: Fixture, task: iced::Task<Message>) {
    let landed = outputs(task);
    assert_eq!(landed.len(), 1, "one completion message");
    let busy = f.app.library.writes.turn(&f.root);
    f.app.library.close_library(&f.root);
    assert!(
        f.app.library.library_at(&f.root).is_some(),
        "the close waits"
    );
    drop(busy);

    for message in landed {
        let task = f.app.update(message);
        settle_task(&mut f.app, task);
    }

    assert!(
        f.app.library.library_at(&f.root).is_none(),
        "the close finished"
    );
}

#[test]
fn a_close_waiting_behind_a_row_reload_finishes_when_it_lands() {
    let mut f = fixture();
    let task = f.event(EditorMsg::ReloadRequested);
    assert_close_finishes_when_the_read_lands(f, task);
}

#[test]
fn a_close_waiting_behind_a_tab_lookup_finishes_when_it_lands() {
    let mut f = fixture();
    let task = f.event(EditorMsg::SelectTab(PreviewTab::Preview));
    assert_close_finishes_when_the_read_lands(f, task);
}
