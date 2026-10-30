//! The off-thread reads of the Component Preview editors (#859): nothing is
//! read when planning, and an answer that went stale is dropped.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use signex_library::{
    ComponentRow, LibraryAdapter, LibraryError, Manifest, PrimitiveRef, RowId, Symbol,
};
use uuid::Uuid;

use super::component_preview::test_row;
use super::preview_reads::ReloadApplied;
use super::state::{
    ComponentPreviewState, EditorAddress, LibraryDisplaySettings, LibraryState, OpenLibrary,
    PreviewTab,
};
use super::test_adapters::ScriptedSymbolsAdapter;

/// A library that stores one row and one symbol the test can change, and
/// counts every read.
struct Shelf {
    manifest: Manifest,
    row: Arc<Mutex<ComponentRow>>,
    symbol: Arc<Mutex<Symbol>>,
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
        Ok(self.row.lock().expect("row").clone())
    }

    fn get_symbol(&self, _uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.symbol.lock().expect("symbol").clone())
    }
}

struct Fixture {
    state: LibraryState,
    root: PathBuf,
    library_id: Uuid,
    address: EditorAddress,
    row: Arc<Mutex<ComponentRow>>,
    symbol: Arc<Mutex<Symbol>>,
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

fn mount_shelf(f: &mut Fixture, library_id: Uuid) {
    let manifest = ScriptedSymbolsAdapter::new(library_id).0.manifest().clone();
    f.state
        .set
        .mount(Box::new(Shelf {
            manifest,
            row: f.row.clone(),
            symbol: f.symbol.clone(),
            reads: f.reads.clone(),
        }))
        .expect("mount");
}

/// A mounted library storing row "Stored" and symbol "Disk", and an open
/// preview of a copy of that row that has drawn nothing yet.
fn fixture() -> Fixture {
    let library_id = Uuid::now_v7();
    let root = PathBuf::from("/tmp/preview-reads/Lib.snxlib");
    let mut stored = test_row();
    stored.symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
    stored.description = "Stored".into();
    let address = EditorAddress::new(root.clone(), "R".into(), RowId::from_uuid(stored.row_id));
    let mut copy = stored.clone();
    copy.description = "Old".into();
    let mut f = Fixture {
        state: LibraryState::default(),
        root: root.clone(),
        library_id,
        address: address.clone(),
        row: Arc::new(Mutex::new(stored)),
        symbol: Arc::new(Mutex::new(Symbol::empty("Disk"))),
        reads: Arc::new(AtomicUsize::new(0)),
    };
    mount_shelf(&mut f, library_id);
    open(&mut f.state, &root, library_id);
    f.state.editors.insert(
        address,
        ComponentPreviewState::from_row(root, "R".into(), copy),
    );
    f
}

impl Fixture {
    fn description(&self) -> String {
        self.state.editors[&self.address].row.description.clone()
    }

    fn symbol_name(&self) -> Option<String> {
        self.state.editors[&self.address]
            .symbol
            .as_ref()
            .map(|s| s.name.clone())
    }
}

// ── first visit of a tab ────────────────────────────────────────────────

#[test]
fn planning_a_tab_lookup_reads_nothing_and_the_worker_does() {
    let f = fixture();
    let plan = f
        .state
        .plan_tab_resolve(&f.address, PreviewTab::Preview)
        .expect("a plan");
    assert_eq!(f.reads.load(Ordering::SeqCst), 0);
    let _outcome = plan.read();
    assert_eq!(f.reads.load(Ordering::SeqCst), 1);
}

#[test]
fn a_tab_that_shows_no_primitive_needs_no_lookup() {
    let f = fixture();
    assert!(
        f.state
            .plan_tab_resolve(&f.address, PreviewTab::Parameters)
            .is_none()
    );
}

#[test]
fn a_tab_lookup_fills_the_symbol_when_it_lands() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_tab_resolve(&f.address, PreviewTab::Preview)
        .expect("a plan");
    f.state.apply_tab_resolve(plan.read());
    assert_eq!(f.symbol_name().as_deref(), Some("Disk"));
}

#[test]
fn a_tab_lookup_for_a_preview_rebound_meanwhile_is_dropped() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_tab_resolve(&f.address, PreviewTab::Preview)
        .expect("a plan");
    let outcome = plan.read();
    f.state
        .editors
        .get_mut(&f.address)
        .expect("preview")
        .row
        .symbol_ref = PrimitiveRef::new(f.library_id, Uuid::now_v7());
    f.state.apply_tab_resolve(outcome);
    assert_eq!(f.symbol_name(), None);
}

#[test]
fn a_tab_lookup_never_overwrites_a_symbol_already_held() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_tab_resolve(&f.address, PreviewTab::Preview)
        .expect("a plan");
    let outcome = plan.read();
    f.state.editors.get_mut(&f.address).expect("preview").symbol = Some(Symbol::empty("Held"));
    f.state.apply_tab_resolve(outcome);
    assert_eq!(f.symbol_name().as_deref(), Some("Held"));
}

// ── reloading a row ─────────────────────────────────────────────────────

#[test]
fn planning_a_reload_reads_nothing_and_the_worker_does() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("a plan");
    assert_eq!(f.reads.load(Ordering::SeqCst), 0);
    let _outcome = plan.read();
    assert!(f.reads.load(Ordering::SeqCst) >= 1);
}

#[test]
fn a_reload_replaces_the_copy_and_what_its_tab_shows() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("a plan");
    let applied = f.state.apply_editor_reload(plan.read());
    assert_eq!(applied, ReloadApplied::Replaced { resolve_tab: None });
    assert_eq!(f.description(), "Stored");
    assert_eq!(f.symbol_name().as_deref(), Some("Disk"));
}

#[test]
fn a_reload_superseded_by_a_newer_one_is_dropped() {
    let mut f = fixture();
    let first = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("first");
    let second = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("second");
    let early = first.read();
    f.row.lock().expect("row").description = "Newest".into();
    let late = second.read();

    assert_eq!(
        f.state.apply_editor_reload(late),
        ReloadApplied::Replaced { resolve_tab: None }
    );
    assert_eq!(f.state.apply_editor_reload(early), ReloadApplied::Dropped);
    assert_eq!(f.description(), "Newest");
}

#[test]
fn a_reload_of_a_library_closed_and_opened_again_meanwhile_is_dropped() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("a plan");
    let outcome = plan.read();
    f.state.close_library(&f.root.clone());
    let library_id = f.library_id;
    mount_shelf(&mut f, library_id);
    open(&mut f.state, &f.root.clone(), library_id);
    let mut copy = f.row.lock().expect("row").clone();
    copy.description = "Reopened".into();
    f.state.editors.insert(
        f.address.clone(),
        ComponentPreviewState::from_row(f.root.clone(), "R".into(), copy),
    );

    assert_eq!(f.state.apply_editor_reload(outcome), ReloadApplied::Dropped);
    assert_eq!(f.description(), "Reopened");
}

#[test]
fn a_reload_nobody_asked_for_yields_to_edits_typed_since() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_editor_reload(&f.address, true)
        .expect("a plan");
    let outcome = plan.read();
    let preview = f.state.editors.get_mut(&f.address).expect("preview");
    preview.row.description = "Typed".into();
    preview.dirty = true;

    assert_eq!(f.state.apply_editor_reload(outcome), ReloadApplied::Dropped);
    assert_eq!(f.description(), "Typed");
}

#[test]
fn a_reload_the_user_asked_for_replaces_even_a_dirty_copy() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("a plan");
    let outcome = plan.read();
    let preview = f.state.editors.get_mut(&f.address).expect("preview");
    preview.row.description = "Typed".into();
    preview.dirty = true;

    f.state.apply_editor_reload(outcome);
    assert_eq!(f.description(), "Stored");
}

#[test]
fn no_reload_replaces_a_copy_that_is_being_saved() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("a plan");
    let outcome = plan.read();
    let preview = f.state.editors.get_mut(&f.address).expect("preview");
    preview.saving_row = Some(preview.row.clone());

    assert_eq!(f.state.apply_editor_reload(outcome), ReloadApplied::Dropped);
    assert_eq!(f.description(), "Old");
}

#[test]
fn a_reload_after_a_tab_change_asks_for_the_new_tab_to_be_looked_up() {
    let mut f = fixture();
    let plan = f
        .state
        .plan_editor_reload(&f.address, false)
        .expect("a plan");
    let outcome = plan.read();
    f.state
        .editors
        .get_mut(&f.address)
        .expect("preview")
        .active_tab = PreviewTab::Simulation;

    assert_eq!(
        f.state.apply_editor_reload(outcome),
        ReloadApplied::Replaced {
            resolve_tab: Some(PreviewTab::Simulation)
        }
    );
}
