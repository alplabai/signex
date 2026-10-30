//! #859 — the Properties panel's symbol and footprint summaries of the
//! selected library row are read by a worker, not inside `update`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use signex_library::{LibraryAdapter, LibraryError, Manifest, PrimitiveRef, Symbol};
use uuid::Uuid;

use crate::app::path_ops::settle_task;
use crate::app::{Signex, TabInfo, TabKind};
use crate::library::component_preview::test_row;
use crate::library::state::{LibraryBrowserState, LibraryDisplaySettings, OpenLibrary};
use crate::library::test_adapters::ScriptedSymbolsAdapter;

/// A library whose `get_symbol` answers `found`, or "not there", and counts
/// the calls.
struct SymbolShelf {
    manifest: Manifest,
    found: Option<Symbol>,
    reads: Arc<AtomicUsize>,
}

impl LibraryAdapter for SymbolShelf {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(Vec::new())
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.found
            .clone()
            .ok_or_else(|| LibraryError::NotFound(uuid.to_string()))
    }
}

/// An app with a Library Browser tab on a mounted library whose one row
/// binds a symbol, and that row selected. `adapter` is the mounted library.
fn app_with_selected_row(
    adapter: impl FnOnce(Uuid, Manifest) -> Box<dyn LibraryAdapter>,
) -> Signex {
    let library_id = Uuid::now_v7();
    let manifest = ScriptedSymbolsAdapter::new(library_id).0.manifest().clone();
    let root = PathBuf::from("/tmp/row-summary/Lib.snxlib");
    let (mut app, _task) = Signex::new();
    app.library.open_libraries.clear();
    app.library
        .set
        .mount(adapter(library_id, manifest))
        .expect("mount");
    let mut row = test_row();
    row.symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
    let row_id = signex_library::RowId::from_uuid(row.row_id);
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Lib".into(),
        library_id,
        tables: std::collections::HashMap::from([("R".to_string(), vec![row])]),
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
    let mut browser = LibraryBrowserState::new(root.clone());
    browser.active_table = Some("R".into());
    browser.selected_row = Some(row_id);
    app.library.library_browsers.insert(root.clone(), browser);
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: root.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::LibraryBrowser(root),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.refresh_panel_ctx();
    app
}

fn shelf(
    found: Option<Symbol>,
) -> (
    impl FnOnce(Uuid, Manifest) -> Box<dyn LibraryAdapter>,
    Arc<AtomicUsize>,
) {
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = reads.clone();
    (
        move |_id, manifest| {
            Box::new(SymbolShelf {
                manifest,
                found,
                reads: counted,
            }) as Box<dyn LibraryAdapter>
        },
        reads,
    )
}

fn symbol_summary(app: &Signex) -> String {
    app.document_state
        .panel_ctx
        .library_row_detail
        .as_ref()
        .expect("a row is selected")
        .symbol_summary
        .clone()
}

#[test]
fn the_panel_reads_no_symbol_inside_update_and_shows_it_once_the_task_has_run() {
    let (adapter, reads) = shelf(Some(Symbol::empty("Bound")));
    let mut app = app_with_selected_row(adapter);

    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "building the panel reads nothing"
    );
    assert_eq!(symbol_summary(&app), "Reading symbol…");

    let task = app.start_row_summary_read().expect("a read is wanted");
    assert_eq!(reads.load(Ordering::SeqCst), 0, "starting it reads nothing");
    settle_task(&mut app, task);

    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(symbol_summary(&app), "Symbol bound — 0 pins");
}

#[test]
fn an_unresolved_symbol_is_worded_as_unresolved() {
    let (adapter, _reads) = shelf(None);
    let mut app = app_with_selected_row(adapter);

    let task = app.start_row_summary_read().expect("a read is wanted");
    settle_task(&mut app, task);

    assert_eq!(
        symbol_summary(&app),
        "Symbol unresolved (UUID not in mounted libraries)"
    );
}

#[test]
fn a_symbol_that_cannot_be_read_says_so_and_is_not_called_unresolved() {
    let mut app = app_with_selected_row(|id, _| Box::new(ScriptedSymbolsAdapter::new(id).0));

    let task = app.start_row_summary_read().expect("a read is wanted");
    settle_task(&mut app, task);

    assert!(
        symbol_summary(&app).starts_with("Symbol could not be read from its library"),
        "{}",
        symbol_summary(&app)
    );
}

#[test]
fn an_answer_is_read_once_not_on_every_panel_rebuild() {
    let (adapter, reads) = shelf(Some(Symbol::empty("Bound")));
    let mut app = app_with_selected_row(adapter);
    let task = app.start_row_summary_read().expect("a read is wanted");
    settle_task(&mut app, task);

    app.refresh_panel_ctx();
    app.refresh_panel_ctx();

    assert!(
        app.start_row_summary_read().is_none(),
        "nothing to read again"
    );
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn a_read_superseded_by_a_newer_one_is_dropped() {
    let (adapter, _reads) = shelf(Some(Symbol::empty("Bound")));
    let mut app = app_with_selected_row(adapter);
    let first = app.start_row_summary_read().expect("first read");
    // The library's data changed (a saved primitive): the key moved on.
    app.library.touch_data();
    app.refresh_panel_ctx();
    let second = app.start_row_summary_read().expect("second read");

    settle_task(&mut app, first);
    assert_eq!(
        symbol_summary(&app),
        "Reading symbol…",
        "the superseded read answers nothing"
    );

    settle_task(&mut app, second);
    assert_eq!(symbol_summary(&app), "Symbol bound — 0 pins");
}

/// A close asked while the read runs waits behind it; the message that
/// lands the read must finish it.
#[test]
fn a_close_waiting_behind_the_summary_read_finishes_when_it_lands() {
    let (adapter, _reads) = shelf(Some(Symbol::empty("Bound")));
    let mut app = app_with_selected_row(adapter);
    let root = app.library.open_libraries[0].root.clone();
    let task = app.start_row_summary_read().expect("a read is wanted");
    let landed = crate::persist::run_task::outputs(task);
    assert_eq!(landed.len(), 1, "one completion message");
    let busy = app.library.writes.turn(&root);
    app.library.close_library(&root);
    assert!(app.library.library_at(&root).is_some(), "the close waits");
    drop(busy);

    for message in landed {
        let task = app.update(message);
        settle_task(&mut app, task);
    }

    assert!(
        app.library.library_at(&root).is_none(),
        "the close finished"
    );
}
