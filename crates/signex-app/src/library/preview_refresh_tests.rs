//! Re-resolving open Component Previews after a primitive save (#859):
//! the plan reads nothing, the read runs apart from state, and a readout
//! that went stale is dropped.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use signex_library::{LibraryAdapter, LibraryError, PrimitiveRef, RowId, Symbol};
use uuid::Uuid;

use super::component_preview::test_row;
use super::state::{
    ComponentPreviewState, EditorAddress, LibraryDisplaySettings, LibraryState, OpenLibrary,
};
use super::test_adapters::ScriptedSymbolsAdapter;

/// A library whose `get_symbol` answers with whatever the test last stored
/// and counts the calls.
struct SymbolStoreAdapter {
    manifest: signex_library::Manifest,
    symbol: Arc<Mutex<Symbol>>,
    reads: Arc<AtomicUsize>,
}

impl LibraryAdapter for SymbolStoreAdapter {
    fn manifest(&self) -> &signex_library::Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(Vec::new())
    }

    fn get_symbol(&self, _uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.symbol.lock().expect("symbol lock").clone())
    }
}

struct Fixture {
    state: LibraryState,
    root: PathBuf,
    address: EditorAddress,
    stored: Arc<Mutex<Symbol>>,
    reads: Arc<AtomicUsize>,
}

fn open(state: &mut LibraryState, root: &std::path::Path, library_id: Uuid) {
    state.open_libraries.push(OpenLibrary {
        root: root.to_path_buf(),
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
}

/// A mounted library holding symbol "Old", and a preview of a row bound to
/// it that already resolved that symbol.
fn fixture() -> Fixture {
    let library_id = Uuid::now_v7();
    let root = PathBuf::from("/tmp/preview-refresh/Lib.snxlib");
    let mut state = LibraryState::default();
    let manifest = ScriptedSymbolsAdapter::new(library_id).0;
    let stored = Arc::new(Mutex::new(Symbol::empty("Old")));
    let reads = Arc::new(AtomicUsize::new(0));
    state
        .set
        .mount(Box::new(SymbolStoreAdapter {
            manifest: manifest_of(manifest),
            symbol: stored.clone(),
            reads: reads.clone(),
        }))
        .expect("mount");
    open(&mut state, &root, library_id);
    let mut row = test_row();
    row.symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
    let address = EditorAddress::new(root.clone(), "R".into(), RowId::from_uuid(row.row_id));
    let mut preview = ComponentPreviewState::from_row(root.clone(), "R".into(), row);
    preview.symbol = Some(Symbol::empty("Old"));
    state.editors.insert(address.clone(), preview);
    Fixture {
        state,
        root,
        address,
        stored,
        reads,
    }
}

fn manifest_of(adapter: ScriptedSymbolsAdapter) -> signex_library::Manifest {
    adapter.manifest().clone()
}

impl Fixture {
    fn store(&self, name: &str) {
        *self.stored.lock().expect("symbol lock") = Symbol::empty(name);
    }

    fn shown(&self) -> String {
        self.state.editors[&self.address]
            .symbol
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_default()
    }
}

#[test]
fn planning_a_preview_refresh_reads_no_primitive() {
    let mut fx = fixture();
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");

    let plan = fx.state.plan_preview_refresh(&saved).expect("a plan");

    assert_eq!(fx.reads.load(Ordering::SeqCst), 0, "update only plans");
    let _readout = plan.read();
    assert_eq!(fx.reads.load(Ordering::SeqCst), 1, "the worker reads");
}

#[test]
fn a_fresh_readout_replaces_the_resolved_symbol() {
    let mut fx = fixture();
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");
    let plan = fx.state.plan_preview_refresh(&saved).expect("a plan");
    fx.store("New");

    fx.state.apply_preview_readout(plan.read());

    assert_eq!(fx.shown(), "New");
}

#[test]
fn a_readout_superseded_by_a_newer_plan_is_dropped() {
    let mut fx = fixture();
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");
    fx.store("Middle");
    let first = fx.state.plan_preview_refresh(&saved).expect("first plan");
    let second = fx.state.plan_preview_refresh(&saved).expect("second plan");
    let early = first.read();
    fx.store("Newest");
    let late = second.read();

    fx.state.apply_preview_readout(late);
    fx.state.apply_preview_readout(early);

    assert_eq!(fx.shown(), "Newest", "the older read must not win");
}

#[test]
fn a_readout_for_a_library_closed_meanwhile_is_dropped() {
    let mut fx = fixture();
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");
    let plan = fx.state.plan_preview_refresh(&saved).expect("a plan");
    fx.store("New");
    let readout = plan.read();
    fx.state.open_libraries.clear();

    fx.state.apply_preview_readout(readout);

    assert_eq!(fx.shown(), "Old");
}

#[test]
fn a_preview_that_was_rebound_meanwhile_keeps_its_symbol() {
    let mut fx = fixture();
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");
    let plan = fx.state.plan_preview_refresh(&saved).expect("a plan");
    fx.store("New");
    let readout = plan.read();
    let library_id = fx.state.open_libraries[0].library_id;
    fx.state
        .editors
        .get_mut(&fx.address)
        .expect("preview")
        .row
        .symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());

    fx.state.apply_preview_readout(readout);

    assert_eq!(fx.shown(), "Old");
}

#[test]
fn a_failed_lookup_keeps_the_previous_copy() {
    let mut fx = fixture();
    // Swap in an adapter whose `get_symbol` fails.
    let library_id = fx.state.open_libraries[0].library_id;
    let mut state = LibraryState::default();
    let (adapter, _) = ScriptedSymbolsAdapter::new(library_id);
    state.set.mount(Box::new(adapter)).expect("mount");
    state.open_libraries = std::mem::take(&mut fx.state.open_libraries);
    state.editors = std::mem::take(&mut fx.state.editors);
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");
    let plan = state.plan_preview_refresh(&saved).expect("a plan");

    state.apply_preview_readout(plan.read());

    let kept = state.editors[&fx.address]
        .symbol
        .as_ref()
        .map(|s| s.name.clone());
    assert_eq!(kept.as_deref(), Some("Old"));
}

#[test]
fn a_preview_that_never_resolved_stays_lazy() {
    let mut fx = fixture();
    fx.state
        .editors
        .get_mut(&fx.address)
        .expect("preview")
        .symbol = None;
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");

    assert!(fx.state.plan_preview_refresh(&saved).is_none());
}

#[test]
fn a_readout_planned_under_a_closed_mount_does_not_land_on_the_next_mount() {
    let mut fx = fixture();
    let saved = fx.root.parent().unwrap().join("symbols/a.snxsym");
    let plan = fx.state.plan_preview_refresh(&saved).expect("a plan");
    fx.store("New");
    let readout = plan.read();
    let library_id = fx.state.open_libraries[0].library_id;
    let row = fx.state.editors[&fx.address].row.clone();
    fx.state.close_library(&fx.root.clone());
    // The same library (same id, same path) is opened again, with a
    // preview of its own that resolved "Old".
    let manifest = manifest_of(ScriptedSymbolsAdapter::new(library_id).0);
    fx.state
        .set
        .mount(Box::new(SymbolStoreAdapter {
            manifest,
            symbol: fx.stored.clone(),
            reads: fx.reads.clone(),
        }))
        .expect("remount");
    open(&mut fx.state, &fx.root, library_id);
    let mut preview = ComponentPreviewState::from_row(fx.root.clone(), "R".into(), row);
    preview.symbol = Some(Symbol::empty("Old"));
    fx.state.editors.insert(fx.address.clone(), preview);

    fx.state.apply_preview_readout(readout);

    assert_eq!(fx.shown(), "Old");
}
