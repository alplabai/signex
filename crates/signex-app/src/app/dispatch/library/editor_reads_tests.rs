//! #859 — the Component Preview editor's reads (a tab's first visit, a row
//! reload) run on a worker; the messages that land them finish a close that
//! was waiting behind them.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use signex_library::{
    ComponentRow, LibraryAdapter, LibraryError, LifecycleState, Manifest, PrimitiveRef, RowId,
    Symbol,
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

/// A real edit of the row, as the Lifecycle pills send it.
fn edit(f: &mut Fixture, state: LifecycleState) {
    let task = f.event(EditorMsg::SetLifecycle(state));
    settle_task(&mut f.app, task);
}

#[test]
fn a_reload_asked_of_a_clean_copy_keeps_what_is_typed_before_it_lands() {
    let mut f = fixture();

    let task = f.event(EditorMsg::ReloadRequested);
    // The read is still queued; the user types.
    edit(&mut f, LifecycleState::Obsolete);
    settle_task(&mut f.app, task);

    assert_eq!(f.preview().row.state, LifecycleState::Obsolete);
    assert!(f.preview().dirty);
}

#[test]
fn a_confirmed_reload_keeps_what_is_typed_after_the_confirm() {
    let mut f = fixture();
    edit(&mut f, LifecycleState::Deprecated);

    let task = f.event(EditorMsg::ReloadConfirmed);
    edit(&mut f, LifecycleState::Obsolete);
    settle_task(&mut f.app, task);

    assert_eq!(f.preview().row.state, LifecycleState::Obsolete);
}

#[test]
fn a_confirmed_reload_keeps_a_simulation_edit_typed_after_the_confirm() {
    let mut f = fixture();
    edit(&mut f, LifecycleState::Deprecated);
    let preview = f.app.library.editors.get_mut(&f.address).expect("preview");
    preview.sim = Some(signex_library::SimModel::empty(
        "Deck",
        signex_library::SimKind::Spice3,
    ));
    preview.sim_body = Some(iced::widget::text_editor::Content::with_text("*"));

    let task = f.event(EditorMsg::ReloadConfirmed);
    let typed =
        iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert('x'));
    let task_edit = f.event(EditorMsg::SimBodyAction(typed));
    settle_task(&mut f.app, task_edit);
    settle_task(&mut f.app, task);

    let sim = f.preview().sim.as_ref().expect("the edited model is kept");
    assert!(sim.body.contains('x'), "{:?}", sim.body);
    assert_eq!(f.preview().row.state, LifecycleState::Deprecated);
}

#[test]
fn a_confirmed_reload_replaces_the_edits_that_existed_at_the_confirm() {
    let mut f = fixture();
    edit(&mut f, LifecycleState::Deprecated);

    let task = f.event(EditorMsg::ReloadConfirmed);
    settle_task(&mut f.app, task);

    assert_eq!(f.preview().row.state, LifecycleState::Draft);
}

/// A reload is requested of a clean copy, then something else changes the
/// preview. `settle` runs the reload to the end; the preview must keep what
/// the other change made.
#[test]
fn an_alternate_symbol_picked_after_a_confirmed_reload_is_kept() {
    let mut f = fixture();
    edit(&mut f, LifecycleState::Deprecated);
    let library_id = f.preview().row.symbol_ref.library_id;

    let pick = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    let task = f.event(EditorMsg::ReloadConfirmed);
    // The pick's symbols arrive first (it holds the earlier read turn); the
    // reload then finds the edit.
    settle_task(&mut f.app, pick);
    settle_task(&mut f.app, task);

    assert_eq!(
        f.preview().row.alternate_symbols.len(),
        1,
        "the alternate is kept"
    );
    assert!(f.preview().dirty);
}

/// The alternate is one undo step: undoing the edit after it must not land
/// on a row that reads clean while it still holds the unsaved alternate.
#[test]
fn undoing_an_edit_made_after_an_alternate_add_stays_dirty() {
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;
    let pick = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    settle_task(&mut f.app, pick);
    assert_eq!(f.preview().row.alternate_symbols.len(), 1);
    edit(&mut f, LifecycleState::Deprecated);

    let preview = f.app.library.editors.get_mut(&f.address).unwrap();
    assert!(preview.step_history(true));
    assert_eq!(preview.row.alternate_symbols.len(), 1);
    assert!(preview.dirty, "the alternate is not on disk");
    assert!(preview.step_history(true), "the add itself is undoable");
    assert!(preview.row.alternate_symbols.is_empty());
    assert!(!preview.dirty, "back at the opened row");
}

#[test]
fn a_failed_bind_write_leaves_the_row_dirty() {
    let mut f = fixture();
    f.app
        .library
        .editors
        .get_mut(&f.address)
        .unwrap()
        .restart_history();
    assert!(!f.preview().dirty);
    let failure = crate::library::writes::WriteFailure {
        reason: "read-only".into(),
        row_changed: false,
    };
    let _ = f.app.preview_row_bound(f.address.clone(), Err(failure));
    assert!(f.preview().dirty);
    assert!(f.preview().history.is_dirty());
}

#[test]
fn picking_an_alternate_symbol_reads_nothing_in_update() {
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;

    let task = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );

    assert_eq!(f.reads.load(Ordering::SeqCst), 0, "update only plans");
    assert!(f.preview().row.alternate_symbols.is_empty());
    settle_task(&mut f.app, task);
    assert_eq!(f.preview().row.alternate_symbols.len(), 1);
    assert!(f.reads.load(Ordering::SeqCst) >= 2, "both symbols read");
}

/// The row is edited while the two symbols are read: the pin check ran
/// against a row that is gone, so the pick is refused out loud rather than
/// applied.
#[test]
fn an_alternate_pick_is_refused_when_the_row_was_edited_while_it_was_read() {
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;

    let pick = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    edit(&mut f, LifecycleState::Obsolete);
    settle_task(&mut f.app, pick);

    assert!(f.preview().row.alternate_symbols.is_empty(), "not applied");
    let why = f.preview().alternate_error.as_deref().expect("said so");
    assert!(why.contains("edited while"), "{why}");
    assert_eq!(f.preview().row.state, LifecycleState::Obsolete);
}

#[test]
fn an_alternate_pick_does_not_land_on_a_preview_closed_while_it_was_read() {
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;

    let pick = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    // The preview is closed and reopened, as the same address, before the
    // answer arrives: the answer belongs to the preview that was closed.
    f.app.library.editors.remove(&f.address);
    f.app.library.alternate_generations.clear();
    let fresh = ComponentPreviewState::from_row(f.root.clone(), "R".into(), test_row());
    f.app.library.editors.insert(f.address.clone(), fresh);
    settle_task(&mut f.app, pick);

    assert!(f.preview().row.alternate_symbols.is_empty());
}

#[test]
fn an_alternate_pick_for_a_preview_closed_while_it_was_read_says_so() {
    let needle = "Alternate symbol not added: the component preview was closed";
    let before = crate::diagnostics::logged_count(needle);
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;

    let pick = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    // A real close also forgets the pick's generation.
    let address = f.address.clone();
    let close = f.app.close_component_preview(&address);
    settle_task(&mut f.app, close);
    settle_task(&mut f.app, pick);

    assert_eq!(
        crate::diagnostics::logged_count(needle),
        before + 1,
        "the closed preview is reported"
    );
}

#[test]
fn a_second_alternate_pick_supersedes_the_first_still_in_flight() {
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;
    let first = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    let second = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );

    settle_task(&mut f.app, first);
    assert!(f.preview().row.alternate_symbols.is_empty(), "stale pick");
    settle_task(&mut f.app, second);
    assert_eq!(f.preview().row.alternate_symbols.len(), 1);
}

#[test]
fn a_bind_started_after_a_reload_was_asked_is_not_undone_by_it() {
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;
    let picked = PrimitiveRef::new(library_id, Uuid::now_v7());

    let reload = f.event(EditorMsg::ReloadRequested);
    let bind = f.app.apply_primitive_pick_to_preview(
        f.address.clone(),
        signex_library::PrimitiveKind::Symbol,
        picked,
    );
    // The bind finishes before the read lands.
    let preview = f.app.library.editors.get_mut(&f.address).expect("preview");
    preview.binding_primitive = false;
    settle_task(&mut f.app, reload);

    assert_eq!(
        f.preview().row.symbol_ref,
        picked,
        "the bound symbol stands"
    );
    assert_eq!(
        f.preview().row.description,
        "Old",
        "the stale read was not applied"
    );
    settle_task(&mut f.app, bind);
}

#[test]
fn a_footprint_canvas_event_after_a_requested_reload_does_not_stop_it() {
    let mut f = fixture();

    let task = f.event(EditorMsg::ReloadRequested);
    let moved = f.event(EditorMsg::Footprint(
        crate::library::messages::FootprintEditorMsg::CursorAt {
            x_mm: 1.0,
            y_mm: 2.0,
            px_per_mm: 30.0,
        },
    ));
    settle_task(&mut f.app, moved);
    settle_task(&mut f.app, task);

    assert_eq!(f.preview().row.description, "Stored", "the reload applied");
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

#[test]
fn an_alternate_symbol_picked_after_the_confirm_survives_the_reload() {
    let mut f = fixture();
    edit(&mut f, LifecycleState::Deprecated);
    let library_id = f.preview().row.symbol_ref.library_id;
    let task = f.event(EditorMsg::ReloadConfirmed);
    let pick = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    settle_task(&mut f.app, task);
    settle_task(&mut f.app, pick);
    assert_eq!(f.preview().row.alternate_symbols.len(), 1);
}

#[test]
fn an_alternate_pick_planned_against_another_default_symbol_is_refused() {
    let mut f = fixture();
    let library_id = f.preview().row.symbol_ref.library_id;
    let pick = f.app.add_alternate_symbol_from_pick(
        f.address.clone(),
        PrimitiveRef::new(library_id, Uuid::now_v7()),
    );
    // The row's default symbol changes under the read (a reload or a rebase).
    f.app
        .library
        .editors
        .get_mut(&f.address)
        .expect("preview")
        .row
        .symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
    settle_task(&mut f.app, pick);
    assert!(f.preview().row.alternate_symbols.is_empty());
    assert!(f.preview().alternate_error.is_some());
}
