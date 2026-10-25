//! #950 — a placed symbol's `library_version` stamp stays consistent with
//! its embedded library copy. "Update from library" and the Library Updates
//! modal's Apply share one off-thread read (`read_symbols`): the worker reads
//! each instance's row fresh and stamps the version it read, or says why not.

use crate::library_place_symbol::{
    Fixture, app_with_libraries, app_with_library, cross_library, doc, fixture, place,
    place_across, two_unit_symbol,
};
use crate::library_symbol_drift::{
    add_pin, check_drift, click_update, edit_library_symbol, is_drifted, pin_count, run_task,
    select_first,
};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::library::drift::{
    ApplyInput, MountedLibrary, SymbolsRead, apply_input, read_symbols,
};
use signex_app::panels::PanelMsg;
use signex_library::primitive::SymbolFile;
use signex_types::schematic::{Point, SymbolTransform, Wire};
use uuid::Uuid;

// ---- #950: content hash is the truth, library_version a stamp both paths move ----

fn version(app: &Signex) -> String {
    doc(app).symbols[0].library_version.clone()
}

/// Make the placed symbol look stale by version, as after a library bump.
fn make_version_stale(app: &mut Signex) {
    let engine = app.document_state.active_engine_mut().unwrap();
    let mut d = engine.document().clone();
    d.symbols[0].library_version = "0.0.1".into();
    engine.set_document(d);
    reload_engine(app);
}

/// Replace the active engine by a fresh one over the same document: what a
/// sheet looks like right after it is opened, with no undo history.
fn reload_engine(app: &mut Signex) {
    let path = app.document_state.active_path.clone().unwrap();
    let document = doc(app).clone();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new_with_path(document, Some(path)).unwrap(),
    );
}

fn wire_pin_1(app: &mut Signex) {
    let tip = {
        let sheet = doc(app);
        let sym = &sheet.symbols[0];
        let lib = &sheet.lib_symbols[&sym.lib_id];
        let pin1 = lib.pins.iter().find(|p| p.pin.number == "1").unwrap();
        SymbolTransform::from_symbol(sym).apply(pin1.pin.position)
    };
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: Wire {
                start: tip,
                end: Point::new(tip.x - 10.0, tip.y),
                ..Default::default()
            },
        })
        .unwrap();
}

fn scan(app: &mut Signex) {
    let sheet = app.document_state.active_path.clone().unwrap();
    app.scan_library_updates_for_open_schematic(sheet);
}

fn team_fixture() -> Fixture {
    let f = fixture();
    to_team(&f);
    f
}

fn to_team(f: &Fixture) {
    let text = std::fs::read_to_string(&f.snxlib).unwrap();
    assert!(text.contains("mode = \"personal\""));
    std::fs::write(
        &f.snxlib,
        text.replace("mode = \"personal\"", "mode = \"team\""),
    )
    .unwrap();
}

/// Click Apply in the modal and return the handler's own `Task`.
fn click_apply(app: &mut Signex) -> iced::Task<Message> {
    app.update(Message::Library(LibraryMessage::LibraryUpdatesApply))
}

/// Click Apply but build the worker input by hand and leave it un-run, so
/// the library can move on first.
fn click_apply_input(app: &mut Signex) -> (iced::Task<Message>, ApplyInput) {
    let sheet = app.document_state.active_path.clone().unwrap();
    let input = apply_input(
        app.library.library_updates.as_ref().unwrap(),
        app.document_state.engines[&sheet].document(),
    )
    .unwrap();
    (click_apply(app), input)
}

fn mounted(app: &Signex) -> Vec<MountedLibrary> {
    app.library
        .open_libraries
        .iter()
        .map(|l| MountedLibrary {
            id: l.library_id,
            path: l.root.clone(),
            display_name: l.display_name.clone(),
        })
        .collect()
}

fn deliver(app: &mut Signex, read: SymbolsRead) {
    let _ = app.update(Message::Library(LibraryMessage::LibraryUpdateSymbolsRead(
        Box::new(read),
    )));
}

fn select_all_updates(app: &mut Signex) {
    for e in &mut app.library.library_updates.as_mut().unwrap().entries {
        e.selected = true;
    }
}

/// Select everything and click Apply; the handler's `Task` runs on a tokio
/// runtime and its completion is fed back to `update`.
fn apply_team_updates(app: &mut Signex) {
    select_all_updates(app);
    let task = click_apply(app);
    for message in run_task(task) {
        let _ = app.update(message);
    }
}

fn skipped(app: &Signex) -> bool {
    let sheet = app.document_state.active_path.clone().unwrap();
    app.library.skipped_updates_for.contains(&sheet)
}

#[test]
fn update_from_library_also_stamps_the_version() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    let id = select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    click_update(&mut app, id);
    assert_eq!(version(&app), "1.2.3");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(version(&app), "0.0.1", "stamp shares the undo step");
}

#[test]
fn personal_auto_apply_adopts_the_copy_and_leaves_no_drift() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    assert_eq!(pin_count(&app), before + 1);
    assert_eq!(version(&app), "1.2.3");
    assert!(app.library.library_updates.is_none());
    check_drift(&mut app);
    assert!(!is_drifted(&app));
}

#[test]
fn personal_auto_apply_refusal_leaves_version_and_copy() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    wire_pin_1(&mut app);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, |s| s.pins.retain(|p| p.number != "1"));
    scan(&mut app);
    assert_eq!(pin_count(&app), before);
    assert_eq!(version(&app), "0.0.1");
    assert!(skipped(&app), "the refused update stays flagged");
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("update refused") && e.message.contains("U1 pin 1"));
    assert!(logged, "the refusal must reach the Messages panel");
}

#[test]
fn team_apply_adopts_the_copy_and_leaves_no_update_or_drift() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    // The real open order: the update scan first, the drift pass after.
    scan(&mut app);
    check_drift(&mut app);
    assert!(app.library.library_updates.is_some());
    apply_team_updates(&mut app);
    assert_eq!(pin_count(&app), before + 1);
    assert_eq!(version(&app), "1.2.3");
    check_drift(&mut app);
    assert!(!is_drifted(&app));
    scan(&mut app);
    assert!(
        app.library.library_updates.is_none(),
        "no update offered again"
    );
}

#[test]
fn team_apply_refusal_leaves_version_and_copy() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    wire_pin_1(&mut app);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, |s| s.pins.retain(|p| p.number != "1"));
    scan(&mut app);
    check_drift(&mut app);
    apply_team_updates(&mut app);
    assert_eq!(pin_count(&app), before);
    assert_eq!(version(&app), "0.0.1");
    assert!(skipped(&app), "the refused update stays flagged");
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("update refused") && e.message.contains("U1 pin 1"));
    assert!(logged, "the refusal must reach the Messages panel");
}

/// The update scan runs before the drift pass lands, so Apply cannot rely on
/// the pass: it must still bring the copy along with the version (#950).
#[test]
fn team_apply_before_the_drift_result_still_adopts_the_copy() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    assert!(app.library.drift.drifted.is_empty(), "no pass has landed");
    apply_team_updates(&mut app);
    assert_eq!(pin_count(&app), before + 1);
    assert_eq!(version(&app), "1.2.3");
    check_drift(&mut app);
    assert!(!is_drifted(&app));
}

/// Apply goes through the mutation gateway: the canvas snapshot, dirty flag
/// and netlist see the new pin straight away.
#[test]
fn team_apply_syncs_canvas_dirty_flag_and_netlist() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    app.document_state.tabs[0].dirty = false;
    apply_team_updates(&mut app);
    let canvas = app.interaction_state.active_canvas();
    let snap = canvas
        .render_cache
        .as_ref()
        .expect("canvas synced")
        .snapshot();
    assert_eq!(
        snap.lib_symbols[&snap.symbols[0].lib_id].pins.len(),
        before + 1
    );
    assert!(app.document_state.tabs[0].dirty);
    assert!(app.ui_state.project_netlist.is_some(), "netlist refreshed");
}

/// Instances sharing a `lib_id` share one embedded copy and one checkbox:
/// ticking either selects both, and Apply stamps both.
#[test]
fn team_apply_stamps_every_instance_of_the_shared_symbol() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        let mut twin = d.symbols[0].clone();
        twin.uuid = Uuid::new_v4();
        twin.reference = "U2".into();
        d.symbols.push(twin);
        engine.set_document(d);
    }
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    let state = app.library.library_updates.as_ref().unwrap();
    assert_eq!(state.entries.len(), 2);
    assert!(state.entries.iter().all(|e| !e.selected), "major bump: off");
    let first = state.entries[0].symbol_uuid;
    let _ = app.update(Message::Library(
        LibraryMessage::LibraryUpdatesToggleSelection(first),
    ));
    let state = app.library.library_updates.as_ref().unwrap();
    assert!(state.entries.iter().all(|e| e.selected), "one shared box");
    apply_team_updates(&mut app);
    assert!(
        doc(&app)
            .symbols
            .iter()
            .all(|s| s.library_version == "1.2.3")
    );
    assert!(!skipped(&app), "everything applied");
}

/// A row re-bound to a different symbol must not write that symbol into the
/// placed symbol's `lib_id`, which every instance of the old one shares.
#[test]
fn a_rebound_row_does_not_overwrite_the_placed_symbol_copy() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    // The placed symbol keys another symbol than the one its row names.
    let other = format!("{}:{}", f.library_id, Uuid::new_v4());
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        let old = d.symbols[0].lib_id.clone();
        let copy = d.lib_symbols[&old].clone();
        d.lib_symbols.insert(other.clone(), copy);
        d.symbols[0].lib_id = other.clone();
        engine.set_document(d);
    }
    let before = pin_count(&app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    assert_eq!(pin_count(&app), before, "copy untouched");
    assert_eq!(version(&app), "0.0.1");
    let logged = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("different symbol") && e.message.contains(&f.row_id.to_string())
    });
    assert!(logged, "the refusal must reach the Messages panel");
}

/// Open-time maintenance is not the user's edit: no undo step for it.
#[test]
fn personal_auto_apply_at_open_leaves_no_undo_step() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    let steps = app.document_state.active_engine().unwrap().undo_position();
    scan(&mut app);
    assert_eq!(version(&app), "1.2.3");
    assert_eq!(
        app.document_state.active_engine().unwrap().undo_position(),
        steps
    );
}

// ---- #950 round 3: off-thread Apply, re-bound rows, parked sheets ----

/// Apply returns a `Task` and reads nothing inline; the update lands when the
/// worker's result is delivered.
#[test]
fn team_apply_reads_off_thread_and_applies_on_completion() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let task = click_apply(&mut app);
    assert!(task.units() > 0, "the read runs as a Task");
    assert!(app.library.library_updates.is_none(), "modal is taken");
    assert_eq!(pin_count(&app), before, "nothing read or applied inline");
    assert_eq!(version(&app), "0.0.1");
    for message in run_task(task) {
        let _ = app.update(message);
    }
    assert_eq!(pin_count(&app), before + 1);
    assert_eq!(version(&app), "1.2.3");
}

/// A result landing after its sheet closed is logged and skipped.
#[test]
fn team_apply_completion_after_the_sheet_closed_is_skipped() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let (_task, input) = click_apply_input(&mut app);
    let mut read = read_symbols(input, &mounted(&app));
    read.sheet = std::path::PathBuf::from("closed-meanwhile.snxsch");
    deliver(&mut app, read);
    assert_eq!(pin_count(&app), before);
    assert_eq!(version(&app), "0.0.1");
    let logged = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("closed-meanwhile.snxsch")
            && e.message.contains("closed, reloaded or renamed")
    });
    assert!(logged, "the skip must be logged");
}

/// Rows binding one symbol keep their own versions on the shared copy.
#[test]
fn rows_of_one_symbol_keep_their_own_versions() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    // A second row of the same symbol at another version.
    let adapter = signex_library::adapters::local_git::LocalGitAdapter::open(&f.snxlib).unwrap();
    use signex_library::adapter::LibraryAdapter as _;
    let mut row2 = adapter
        .read_row("ICs", signex_library::RowId::from_uuid(f.row_id))
        .unwrap();
    let row2_id = Uuid::now_v7();
    row2.row_id = row2_id;
    row2.internal_pn = signex_library::identity::InternalPn::new("SNX-TWO-B");
    row2.version = "2.0.0".into();
    adapter.insert_row("ICs", row2, "second row").unwrap();
    drop(adapter);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        let mut twin = d.symbols[0].clone();
        twin.uuid = Uuid::new_v4();
        twin.reference = "U2".into();
        twin.row_id = Some(row2_id);
        twin.library_version = "0.0.1".into();
        d.symbols.push(twin);
        engine.set_document(d);
    }
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    apply_team_updates(&mut app);
    let d = doc(&app);
    assert_eq!(d.symbols[0].library_version, "1.2.3");
    assert_eq!(d.symbols[1].library_version, "2.0.0");
    // Update from library stamps per row too.
    let id = select_first(&mut app);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    check_drift(&mut app);
    click_update(&mut app, id);
    let d = doc(&app);
    assert_eq!(d.symbols[0].library_version, "1.2.3");
    assert_eq!(d.symbols[1].library_version, "2.0.0");
}

/// A row now bound to another symbol has no version to stamp for this one:
/// Update from library refreshes the copy and leaves the instance's stamp.
#[test]
fn update_from_library_does_not_stamp_a_rebound_rows_version() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    let id = select_first(&mut app);
    let adapter = signex_library::adapters::local_git::LocalGitAdapter::open(&f.snxlib).unwrap();
    use signex_library::adapter::LibraryAdapter as _;
    let mut row = adapter
        .read_row("ICs", signex_library::RowId::from_uuid(f.row_id))
        .unwrap();
    row.symbol_ref.uuid = Uuid::new_v4();
    row.version = "9.9.9".into();
    adapter.update_row("ICs", row, "rebind").unwrap();
    drop(adapter);
    let before = pin_count(&app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    assert!(is_drifted(&app));
    let _ = signex_app::diagnostics::init_logging();
    click_update(&mut app, id);
    assert_eq!(pin_count(&app), before + 1, "copy refreshed");
    assert_eq!(version(&app), "0.0.1", "other symbol's version not stamped");
    let row = f.row_id.to_string();
    let logged = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("different symbol")
            && e.message.contains("version not stamped")
            && e.message.contains(&row)
    });
    assert!(logged, "the click must say why the version is left alone");
}

/// The open-time update is not the user's: their later edit is undone on
/// Ctrl+Z, the update stays.
#[test]
fn undo_after_a_personal_auto_apply_keeps_the_update() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    wire_pin_1(&mut app);
    assert_eq!(doc(&app).wires.len(), 1);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).wires.is_empty(), "the user's edit is undone");
    assert_eq!(pin_count(&app), before + 1, "the update survives");
    assert_eq!(version(&app), "1.2.3");
    assert!(!app.document_state.active_engine().unwrap().can_undo());
}

/// Apply for a sheet that is open but not the active tab: updated in place,
/// and its net colour redo branch ends like any recorded edit.
#[test]
fn team_apply_on_a_parked_sheet_updates_it_and_clears_net_color_redo() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let a = app.document_state.active_path.clone().unwrap();
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    // Open B and make it the active tab.
    let b = std::path::PathBuf::from("other-sheet.snxsch");
    app.document_state.engines.insert(
        b.clone(),
        signex_engine::Engine::new_with_path(
            signex_types::schematic::SchematicSheet {
                uuid: Uuid::new_v4(),
                paper_size: "A4".to_string(),
                ..Default::default()
            },
            Some(b.clone()),
        )
        .unwrap(),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "other-sheet".to_string(),
        path: b.clone(),
        cached_document: None,
        pcb_revision: 0,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(b);
    app.ui_state.net_color_redo.insert(a.clone(), Vec::new());
    apply_team_updates(&mut app);
    let parked = app.document_state.engines[&a].document();
    assert_eq!(
        parked.lib_symbols[&parked.symbols[0].lib_id].pins.len(),
        before + 1
    );
    assert_eq!(parked.symbols[0].library_version, "1.2.3");
    assert!(app.document_state.dirty_paths.contains(&a));
    assert!(!app.ui_state.net_color_redo.contains_key(&a));
}

// ---- #950 round 4: map key, scan failure, open identity, apply-time rows ----

/// Edit row `f.row_id` in place.
fn edit_row(f: &Fixture, edit: impl FnOnce(&mut signex_library::ComponentRow)) {
    use signex_library::adapter::LibraryAdapter as _;
    let adapter = signex_library::adapters::local_git::LocalGitAdapter::open(&f.snxlib).unwrap();
    let mut row = adapter
        .read_row("ICs", signex_library::RowId::from_uuid(f.row_id))
        .unwrap();
    edit(&mut row);
    adapter.update_row("ICs", row, "edit").unwrap();
}

/// A correctly bound sibling of the same row is stamped with the row's
/// version; the re-bound instance of X on that row must not be.
#[test]
fn a_bound_sibling_on_the_same_row_does_not_leak_its_version() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    // A second library symbol X, which the row does not bind.
    let x_id = Uuid::new_v4();
    let mut x = two_unit_symbol();
    x.uuid = x_id;
    add_pin(&mut x);
    let dir = f.snxlib.parent().unwrap().join("symbols");
    std::fs::write(
        dir.join("other.snxsym"),
        SymbolFile::from_symbol(x).to_toml_string().unwrap(),
    )
    .unwrap();
    let x_key = format!("{}:{x_id}", f.library_id);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        let y_key = d.symbols[0].lib_id.clone();
        let copy = d.lib_symbols[&y_key].clone();
        d.lib_symbols.insert(x_key.clone(), copy);
        // U2: correctly bound to Y on the row. U1: X on the same row.
        let mut u2 = d.symbols[0].clone();
        u2.uuid = Uuid::new_v4();
        u2.reference = "U2".into();
        d.symbols.push(u2);
        d.symbols[0].lib_id = x_key.clone();
        d.symbols[0].reference = "U1".into();
        d.symbols[0].library_version = "0.0.1".into();
        engine.set_document(d);
    }
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    let u1 = doc(&app).symbols[0].uuid;
    assert!(
        app.library
            .drift
            .is_drifted(&app.document_state.active_path.clone().unwrap(), &x_key)
    );
    click_update(&mut app, u1);
    assert_eq!(version(&app), "0.0.1", "Y's version must not land on X");
}

/// An open-time failure survives a later all-checked Apply.
#[test]
fn a_scan_failure_survives_an_all_checked_apply() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        let mut twin = d.symbols[0].clone();
        twin.uuid = Uuid::new_v4();
        twin.reference = "U2".into();
        // Keys another symbol than its row names: refused at scan time.
        twin.lib_id = format!("{}:{}", f.library_id, Uuid::new_v4());
        twin.library_version = "0.0.1".into();
        d.symbols.push(twin);
        engine.set_document(d);
    }
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    assert!(app.library.library_updates.as_ref().unwrap().scan_failed);
    apply_team_updates(&mut app);
    assert_eq!(version(&app), "1.2.3");
    assert!(skipped(&app), "the refused instance is still pending");
}

/// A completion for a sheet closed and reopened meanwhile is dropped.
#[test]
fn team_apply_completion_after_close_and_reopen_is_skipped() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let messages = run_task(click_apply(&mut app));
    reload_engine(&mut app); // same path, fresh engine
    for message in messages {
        let _ = app.update(message);
    }
    assert_eq!(pin_count(&app), before, "the reopened sheet is untouched");
    assert_eq!(version(&app), "0.0.1");
    let sheet = app.document_state.active_path.clone().unwrap();
    let logged = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("not applied") && e.message.contains(&sheet.display().to_string())
    });
    assert!(logged, "the skip must be logged");
}

/// A row that moved past the version the user approved is not applied: the
/// copy and the stamp stay, the entry stays pending, and Messages say so.
#[test]
fn apply_holds_back_a_row_that_moved_since_the_approval() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let (_task, input) = click_apply_input(&mut app);
    edit_row(&f, |r| r.version = "3.0.0".into());
    let read = read_symbols(input, &mounted(&app));
    deliver(&mut app, read);
    assert_eq!(pin_count(&app), before, "copy not adopted");
    assert_eq!(version(&app), "0.0.1", "not stamped");
    assert!(skipped(&app), "the update stays pending");
    let logged = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message
            .contains("not applied (U1 moved to 3.0.0 since you approved 1.2.3)")
    });
    assert!(logged, "the hold-back must reach the Messages panel");
}

/// A row re-bound after the scan is not stamped at apply time; the shared
/// copy is still refreshed, and the line says the version was not stamped.
#[test]
fn apply_does_not_stamp_a_row_rebound_since_the_scan() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let (_task, input) = click_apply_input(&mut app);
    edit_row(&f, |r| {
        r.symbol_ref.uuid = Uuid::new_v4();
        r.version = "9.9.9".into();
    });
    let read = read_symbols(input, &mounted(&app));
    deliver(&mut app, read);
    assert_eq!(pin_count(&app), before + 1, "copy refreshed");
    assert_eq!(version(&app), "0.0.1");
    assert!(skipped(&app));
    let row = f.row_id.to_string();
    let logged = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("different symbol")
            && e.message.contains("version not stamped")
            && !e.message.contains("update skipped")
            && e.message.contains(&row)
    });
    assert!(logged);
}

/// A worker failure on a closed sheet logs the failure once, not a
/// misleading "closed" line besides.
#[test]
fn a_worker_failure_is_logged_on_completion_only_once() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let (_task, mut input) = click_apply_input(&mut app);
    input.sheet = std::path::PathBuf::from("worker-failure-sheet.snxsch");
    let marker = "library update worker failed: marker-1015d";
    deliver(&mut app, SymbolsRead::failed(&input, marker.into()));
    let lines: Vec<_> = signex_app::diagnostics::recent_entries()
        .into_iter()
        .filter(|e| {
            e.message.contains("marker-1015d") || e.message.contains("worker-failure-sheet")
        })
        .collect();
    assert_eq!(lines.len(), 1, "one line: {lines:?}");
}

fn insert_twin(app: &mut Signex, row_id: Uuid, reference: &str, version: &str) {
    let engine = app.document_state.active_engine_mut().unwrap();
    let mut d = engine.document().clone();
    let mut twin = d.symbols[0].clone();
    twin.uuid = Uuid::new_v4();
    twin.reference = reference.into();
    twin.row_id = Some(row_id);
    twin.library_version = version.into();
    d.symbols.push(twin);
    engine.set_document(d);
}

/// Whether Messages carries a re-bind or not-stamped line naming `row`.
fn logged_about(row: Uuid) -> bool {
    let row = row.to_string();
    signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains(&row)
            && (e.message.contains("different symbol") || e.message.contains("not stamped"))
    })
}

/// A second row of `f`'s symbol in `f`'s library.
fn insert_row_of_same_symbol(f: &Fixture, pn: &str, version: &str, rebind: bool) -> Uuid {
    use signex_library::adapter::LibraryAdapter as _;
    let adapter = signex_library::adapters::local_git::LocalGitAdapter::open(&f.snxlib).unwrap();
    let mut row = adapter
        .read_row("ICs", signex_library::RowId::from_uuid(f.row_id))
        .unwrap();
    let id = Uuid::now_v7();
    row.row_id = id;
    row.internal_pn = signex_library::identity::InternalPn::new(pn);
    row.version = version.into();
    if rebind {
        row.symbol_ref.uuid = Uuid::new_v4();
    }
    adapter.insert_row("ICs", row, "extra row").unwrap();
    id
}

// ---- #950 round 5: one apply path, per-instance verdicts from fresh rows ----

/// "Update from library" is a Task: nothing is read in `update`, and the
/// stamp lands when the worker's result is fed back.
#[test]
fn update_from_library_reads_off_thread_and_applies_on_completion() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    let id = select_first(&mut app);
    let before = pin_count(&app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    let task = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::UpdateSymbolFromLibrary(id),
    )));
    assert!(task.units() > 0, "the read runs as a Task");
    assert_eq!(pin_count(&app), before, "nothing read inline");
    assert_eq!(version(&app), "0.0.1");
    let messages = run_task(task);
    assert!(messages.iter().any(|m| matches!(
        m,
        Message::Library(LibraryMessage::LibraryUpdateSymbolsRead(_))
    )));
    for m in messages {
        let _ = app.update(m);
    }
    assert_eq!(pin_count(&app), before + 1);
    assert_eq!(version(&app), "1.2.3");
    assert!(!is_drifted(&app));
}

/// A different row of the same symbol, placed after the drift pass, is no
/// re-bind: no warning, and its own version is stamped.
#[test]
fn a_different_row_of_the_same_symbol_placed_after_the_pass_is_stamped() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let row2 = insert_row_of_same_symbol(&f, "SNX-TWO-C", "2.0.0", false);
    make_version_stale(&mut app);
    let id = select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    // Placed after the pass.
    insert_twin(&mut app, row2, "U2", "0.0.1");
    click_update(&mut app, id);
    let d = doc(&app);
    assert_eq!(d.symbols[0].library_version, "1.2.3");
    assert_eq!(d.symbols[1].library_version, "2.0.0");
    assert!(!logged_about(row2), "no warning for a correctly bound row");
}

/// The row lives in library A, its symbol in library B: the row is read
/// through A. Team Apply.
#[test]
fn team_apply_reads_a_cross_library_row_through_its_own_library() {
    let _ = signex_app::diagnostics::init_logging();
    let (a, b) = cross_library();
    to_team(&a);
    let mut app = app_with_libraries(&[&a, &b]);
    place_across(&mut app, &a, &b);
    make_version_stale(&mut app);
    let before = pin_count(&app);
    edit_library_symbol(&b, add_pin);
    scan(&mut app);
    apply_team_updates(&mut app);
    assert_eq!(pin_count(&app), before + 1);
    assert_eq!(version(&app), "1.2.3");
    assert!(!skipped(&app), "everything applied");
    assert!(!logged_about(a.row_id));
}

/// Same split, through Update from library.
#[test]
fn update_from_library_reads_a_cross_library_row_through_its_own_library() {
    let _ = signex_app::diagnostics::init_logging();
    let (a, b) = cross_library();
    let mut app = app_with_libraries(&[&a, &b]);
    place_across(&mut app, &a, &b);
    make_version_stale(&mut app);
    let id = select_first(&mut app);
    let before = pin_count(&app);
    edit_library_symbol(&b, add_pin);
    check_drift(&mut app);
    assert!(is_drifted(&app));
    click_update(&mut app, id);
    assert_eq!(pin_count(&app), before + 1);
    assert_eq!(version(&app), "1.2.3");
    assert!(!logged_about(a.row_id));
}

/// A row that is gone, and a library that cannot be read, each get their own
/// line (with the cause), and the shared copy is read regardless.
#[test]
fn missing_rows_and_unreadable_libraries_are_told_apart() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let (_task, input) = click_apply_input(&mut app);
    let mut gone = input.clone();
    gone.wanted[0].1[0].row_id = Uuid::new_v4();
    let read = read_symbols(gone, &mounted(&app));
    let line = &read.reads[0].skipped[0];
    assert!(line.contains("version not stamped") && line.contains("is not in its library"));
    assert!(!line.contains("different symbol"), "{line}");
    assert!(read.reads[0].symbol.is_ok(), "copy still read");
    let mut unreadable = input;
    unreadable.wanted[0].1[0].library_id = Uuid::new_v4();
    let read = read_symbols(unreadable, &mounted(&app));
    let line = &read.reads[0].skipped[0];
    assert!(line.contains("version not stamped") && line.contains("is unreadable"));
    assert!(line.contains("its library is not open"), "{line}");
    assert!(read.reads[0].stamps.is_empty());
}

/// Instances sharing the copy are refreshed regardless: a re-bound one gets
/// no stamp (and a line), a correctly bound one gets its own.
#[test]
fn a_rebound_instance_beside_a_bound_one_is_left_unstamped() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let rebound = insert_row_of_same_symbol(&f, "SNX-TWO-D", "2.0.0", true);
    insert_twin(&mut app, rebound, "U2", "0.0.1");
    make_version_stale(&mut app);
    let id = select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    click_update(&mut app, id);
    let d = doc(&app);
    assert_eq!(d.symbols[0].library_version, "1.2.3");
    assert_eq!(d.symbols[1].library_version, "0.0.1", "re-bound: unstamped");
}

// ---- #950 round 6: mixed modes, every instance, scoped stamps ----

/// U1 (row in the Team library A) and U2 (row in the Personal library B)
/// share one copy: B's auto-apply must not move it past U1's modal entry.
#[test]
fn a_lib_id_with_a_team_instance_is_not_auto_applied_for_its_personal_one() {
    let (a, b) = cross_library();
    to_team(&a);
    let mut app = app_with_libraries(&[&a, &b]);
    place_across(&mut app, &a, &b);
    make_version_stale(&mut app);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        let mut u2 = d.symbols[0].clone();
        u2.uuid = Uuid::new_v4();
        u2.reference = "U2".into();
        u2.library_id = Some(b.library_id);
        u2.row_id = Some(b.row_id);
        u2.library_version = "0.0.1".into();
        d.symbols.push(u2);
        engine.set_document(d);
    }
    let before = pin_count(&app);
    edit_library_symbol(&b, add_pin);
    scan(&mut app);
    assert_eq!(pin_count(&app), before, "the shared copy is not touched");
    assert_eq!(doc(&app).symbols[1].library_version, "0.0.1");
    let state = app.library.library_updates.as_ref().unwrap();
    assert_eq!(state.entries.len(), 2, "both instances go to the modal");
    apply_team_updates(&mut app);
    assert_eq!(pin_count(&app), before + 1);
    assert!(
        doc(&app)
            .symbols
            .iter()
            .all(|s| s.library_version == "1.2.3")
    );
}

/// The worker input covers every instance of a selected `lib_id`, with the
/// approval only on the ones the modal listed.
#[test]
fn apply_input_covers_every_instance_of_the_selected_lib_id() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let row2 = insert_row_of_same_symbol(&f, "SNX-TWO-E", "2.0.0", false);
    // U2's stamp is current: the modal does not list it.
    insert_twin(&mut app, row2, "U2", "2.0.0");
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let state = app.library.library_updates.as_ref().unwrap();
    assert_eq!(state.entries.len(), 1);
    let input = apply_input(state, doc(&app)).unwrap();
    let stamps = &input.wanted[0].1;
    assert_eq!(stamps.len(), 2);
    assert_eq!(stamps[0].approved_version.as_deref(), Some("1.2.3"));
    assert_eq!(stamps[1].approved_version, None);
}

// ---- #950 round 7: conservative mixed-mode rule, hold-back summary ----

/// U1 (row `u1_row`, made before A is mounted, in the Team library A) and U2 (stale,
/// row in the Personal library B) share one copy. Returns the app and the
/// pin count before B's symbol grew a pin.
fn team_beside_stale_personal(u1_row: impl FnOnce(&Fixture) -> Uuid) -> (Signex, usize) {
    let (a, b) = cross_library();
    to_team(&a);
    let u1_row = u1_row(&a);
    let mut app = app_with_libraries(&[&a, &b]);
    place_across(&mut app, &a, &b);
    make_version_stale(&mut app);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        let mut u2 = d.symbols[0].clone();
        u2.uuid = Uuid::new_v4();
        u2.reference = "U2".into();
        u2.library_id = Some(b.library_id);
        u2.row_id = Some(b.row_id);
        u2.library_version = "0.0.1".into();
        d.symbols[0].reference = "U1".into();
        d.symbols[0].row_id = Some(u1_row);
        d.symbols.push(u2);
        engine.set_document(d);
    }
    let before = pin_count(&app);
    edit_library_symbol(&b, add_pin);
    (app, before)
}

fn logged_with(parts: &[&str]) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| parts.iter().all(|p| e.message.contains(p)))
}

/// A Team instance whose row cannot be read must still block the Personal
/// sibling's auto-apply: the shared copy stays untouched.
#[test]
fn an_unreadable_team_row_blocks_the_personal_siblings_auto_apply() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, before) = team_beside_stale_personal(|_| Uuid::new_v4());
    scan(&mut app);
    assert_eq!(pin_count(&app), before, "the shared copy is not touched");
    assert_eq!(doc(&app).symbols[1].library_version, "0.0.1");
    assert!(skipped(&app), "the update stays pending");
    assert!(logged_with(&["U1: its library row is unreadable"]));
}

/// Same for a Team row re-bound to another symbol.
#[test]
fn a_rebound_team_row_blocks_the_personal_siblings_auto_apply() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, before) =
        team_beside_stale_personal(|a| insert_row_of_same_symbol(a, "SNX-TWO-G", "1.0.0", true));
    scan(&mut app);
    assert_eq!(pin_count(&app), before, "the shared copy is not touched");
    assert_eq!(doc(&app).symbols[1].library_version, "0.0.1");
    assert!(skipped(&app));
    assert!(logged_with(&["U1: its row binds a different symbol"]));
}

const HELD_BACK: &str =
    "not applied (U1 moved to 3.0.0 since you approved 1.2.3); U2 left on the old copy.";

/// A readable Team-mode sibling that is not stale still blocks the Personal
/// instance's auto-apply, and the blocker line is logged.
#[test]
fn a_current_team_sibling_blocks_the_stale_personal_instances_auto_apply() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, before) = team_beside_stale_personal(|a| a.row_id);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        d.symbols[0].library_version = "1.2.3".into();
        engine.set_document(d);
    }
    scan(&mut app);
    assert_eq!(pin_count(&app), before, "the shared copy is not touched");
    assert_eq!(doc(&app).symbols[1].library_version, "0.0.1");
    assert!(logged_with(&["U1: its library is Team-mode"]));
}

/// A hold-back leaves the sibling instances unstamped and unadopted, says so
/// in one summary line, and the next scan offers the update again.
#[test]
fn a_hold_back_logs_one_summary_and_the_next_scan_reoffers() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let row2 = insert_row_of_same_symbol(&f, "SNX-TWO-F", "2.0.0", false);
    insert_twin(&mut app, row2, "U2", "2.0.0");
    make_version_stale(&mut app);
    let before = pin_count(&app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let (_task, input) = click_apply_input(&mut app);
    edit_row(&f, |r| r.version = "3.0.0".into());
    let read = read_symbols(input, &mounted(&app));
    assert!(read.reads[0].stamps.is_empty());
    let summaries = read.reads[0]
        .skipped
        .iter()
        .filter(|l| l.contains(HELD_BACK))
        .count();
    assert_eq!(summaries, 1, "{:?}", read.reads[0].skipped);
    assert_eq!(read.reads[0].skipped.len(), 1, "exactly one line");
    deliver(&mut app, read);
    assert_eq!(pin_count(&app), before, "copy not adopted");
    assert_eq!(doc(&app).symbols[0].library_version, "0.0.1");
    assert_eq!(doc(&app).symbols[1].library_version, "2.0.0");
    assert!(logged_with(&[HELD_BACK]));
    scan(&mut app);
    let entries = &app.library.library_updates.as_ref().unwrap().entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].ref_des, "U1");
}

/// Apply on a sheet that is no longer open keeps its pending record.
#[test]
fn apply_after_the_sheet_closed_keeps_the_pending_record() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    make_version_stale(&mut app);
    edit_library_symbol(&f, add_pin);
    scan(&mut app);
    select_all_updates(&mut app);
    let sheet = app.document_state.active_path.clone().unwrap();
    app.document_state.engines.remove(&sheet);
    let _ = click_apply(&mut app);
    assert!(app.library.skipped_updates_for.contains(&sheet));
}
