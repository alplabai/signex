//! #912 — a sheet's embedded `LibSymbol` is compared against the mounted
//! library's current symbol; drift is surfaced on the placed symbol and
//! "Update from library" applies the new copy as one undo step, refusing
//! when a connected pin would vanish.

use crate::library_place_symbol::{
    Fixture, app_with_library, doc, fixture, place, two_unit_symbol,
};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::library::drift::{MountedLibrary, SymbolsRead, read_symbols};
use signex_app::panels::PanelMsg;
use signex_library::primitive::{SymbolFile, SymbolPin};
use signex_types::schematic::{Point, SelectedItem, SelectedKind, SymbolTransform, Wire};
use uuid::Uuid;

/// Request a recheck and run the worker half synchronously through the
/// same `check_pending` -> job -> report path `finish_update` uses (the
/// handler also refreshes the Properties selection info).
fn check_drift(app: &mut Signex) {
    app.library.drift.check_pending = true;
    if let Some(job) = app.take_drift_job() {
        let _ = app.update(Message::Library(LibraryMessage::DriftChecked(Box::new(
            job.run(),
        ))));
    }
}

/// Rewrite the fixture's symbol in place (same uuid) with `edit` applied.
fn edit_library_symbol(f: &Fixture, edit: impl FnOnce(&mut signex_library::primitive::Symbol)) {
    let mut s = two_unit_symbol();
    s.uuid = f.symbol_id;
    edit(&mut s);
    let path = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("two-unit.snxsym");
    std::fs::write(path, SymbolFile::from_symbol(s).to_toml_string().unwrap()).unwrap();
}

fn add_pin(s: &mut signex_library::primitive::Symbol) {
    let mut p = SymbolPin::new("4", "P4");
    p.position = [0.0, 15.24];
    p.part_number = 1;
    s.pins.push(p);
}

fn is_drifted(app: &Signex) -> bool {
    let sheet = app.document_state.active_path.clone().unwrap();
    let lib_id = doc(app).symbols[0].lib_id.clone();
    app.library.drift.is_drifted(&sheet, &lib_id)
}

/// Select the first placed symbol (the next `check_drift` refreshes info).
fn select_first(app: &mut Signex) -> Uuid {
    let id = doc(app).symbols[0].uuid;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Symbol)];
    id
}

fn info(app: &Signex, key: &str) -> Option<String> {
    app.document_state
        .panel_ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

fn click_update(app: &mut Signex, id: Uuid) {
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::UpdateSymbolFromLibrary(id),
    )));
}

fn pin_count(app: &Signex) -> usize {
    let sheet = doc(app);
    sheet.lib_symbols[&sheet.symbols[0].lib_id].pins.len()
}

#[test]
fn an_unchanged_library_is_not_drift() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    check_drift(&mut app);
    assert!(!is_drifted(&app));
    assert!(info(&app, "Library Drift").is_none());
}

#[test]
fn editing_the_library_symbol_is_detected_and_shown_on_the_symbol() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    assert!(is_drifted(&app));
    assert_eq!(info(&app, "Library Drift").as_deref(), Some("Yes"));
}

#[test]
fn properties_show_library_and_symbol_names_when_mounted() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    check_drift(&mut app);
    assert_eq!(info(&app, "Library").as_deref(), Some("lib/TWO-UNIT"));
    // The raw id stays available for the power-port style logic.
    assert!(info(&app, "Library ID").is_some_and(|v| v.contains(':')));
}

#[test]
fn update_from_library_applies_as_one_undo_step() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let before = pin_count(&app);
    app.document_state.tabs[0].dirty = false;
    let id = select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);

    click_update(&mut app, id);
    assert_eq!(pin_count(&app), before + 1);
    assert!(!is_drifted(&app));
    assert!(app.document_state.tabs[0].dirty, "update dirties the tab");
    assert!(info(&app, "Library Drift").is_none());

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(pin_count(&app), before, "one undo restores the old copy");
    // The old copy drifts again once rechecked.
    check_drift(&mut app);
    assert!(is_drifted(&app));
}

#[test]
fn update_is_refused_when_a_connected_pin_would_vanish() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let tip = {
        let sheet = doc(&app);
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
    let before = pin_count(&app);
    let id = select_first(&mut app);
    edit_library_symbol(&f, |s| s.pins.retain(|p| p.number != "1"));
    check_drift(&mut app);
    assert!(is_drifted(&app));
    app.document_state.tabs[0].dirty = false;

    click_update(&mut app, id);
    assert_eq!(pin_count(&app), before, "sheet copy untouched");
    assert!(is_drifted(&app), "drift stays visible");
    assert!(!app.document_state.tabs[0].dirty);
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("update refused") && e.message.contains("U1 pin 1"));
    assert!(logged, "the refusal must reach the Messages panel");
}

#[test]
fn a_placed_symbol_is_not_drift_after_save_and_reopen() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sheet.snxsch");
    app.document_state
        .active_engine_mut()
        .unwrap()
        .save_as(&path)
        .unwrap();
    // The embedded copy went through the real TOML writer and parser.
    *app.document_state.active_engine_mut().unwrap() = signex_engine::Engine::open(&path).unwrap();
    select_first(&mut app);
    check_drift(&mut app);
    assert!(!is_drifted(&app), "a round-tripped copy must hash the same");
}

#[test]
fn unmounting_the_library_drops_names_and_drift() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    assert!(is_drifted(&app));
    assert!(info(&app, "Library").is_some());

    app.library.close_library(&f.snxlib);
    assert!(
        app.library.drift.check_pending,
        "closing requests a recheck"
    );
    check_drift(&mut app);
    assert!(!is_drifted(&app));
    assert!(info(&app, "Library").is_none());
    assert!(info(&app, "Library Drift").is_none());
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert_eq!(app.library.drift.display_name(&lib_id), lib_id);
}

#[test]
fn a_stale_drift_report_is_dropped() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    app.library.drift.check_pending = true;
    let old_job = app.take_drift_job().unwrap();
    let old_report = old_job.run(); // pre-edit: no drift
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app); // newer pass: drifted
    assert!(is_drifted(&app));
    let _ = app.update(Message::Library(LibraryMessage::DriftChecked(Box::new(
        old_report,
    ))));
    assert!(is_drifted(&app), "the older pass must not overwrite");
}

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
    let text = std::fs::read_to_string(&f.snxlib).unwrap();
    assert!(text.contains("mode = \"personal\""));
    std::fs::write(
        &f.snxlib,
        text.replace("mode = \"personal\"", "mode = \"team\""),
    )
    .unwrap();
    f
}

/// Click Apply in the modal. Returns the click's `Task` and the result its
/// worker would produce (the `Task` wraps `read_symbols` in `spawn_blocking`).
fn click_apply(app: &mut Signex) -> (iced::Task<Message>, SymbolsRead) {
    let state = app.library.library_updates.as_ref().unwrap();
    let sheet = state.schematic_path.clone();
    let any_unchecked = state.entries.iter().any(|e| !e.selected);
    let mut wanted: std::collections::BTreeMap<String, Vec<(Uuid, String)>> =
        std::collections::BTreeMap::new();
    for e in state.entries.iter().filter(|e| e.selected) {
        wanted
            .entry(e.lib_id.clone())
            .or_default()
            .push((e.symbol_uuid, e.latest_version.clone()));
    }
    let libs: Vec<MountedLibrary> = app
        .library
        .open_libraries
        .iter()
        .map(|l| MountedLibrary {
            id: l.library_id,
            path: l.root.clone(),
            display_name: l.display_name.clone(),
        })
        .collect();
    let task = app.update(Message::Library(LibraryMessage::LibraryUpdatesApply));
    let read = read_symbols(sheet, any_unchecked, &libs, wanted.into_iter().collect());
    (task, read)
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

fn apply_team_updates(app: &mut Signex) {
    select_all_updates(app);
    let (_task, read) = click_apply(app);
    deliver(app, read);
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
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("different symbol") && e.message.contains(&other));
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
    let (task, read) = click_apply(&mut app);
    assert!(task.units() > 0, "the read runs as a Task");
    assert!(app.library.library_updates.is_none(), "modal is taken");
    assert_eq!(pin_count(&app), before, "nothing read or applied inline");
    assert_eq!(version(&app), "0.0.1");
    deliver(&mut app, read);
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
    let (_task, mut read) = click_apply(&mut app);
    read.sheet = std::path::PathBuf::from("closed-meanwhile.snxsch");
    deliver(&mut app, read);
    assert_eq!(pin_count(&app), before);
    assert_eq!(version(&app), "0.0.1");
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("closed-meanwhile.snxsch") && e.message.contains("closed"));
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
    assert!(app.library.drift.versions.is_empty(), "no version pushed");
    click_update(&mut app, id);
    assert_eq!(pin_count(&app), before + 1, "copy refreshed");
    assert_eq!(version(&app), "0.0.1", "other symbol's version not stamped");
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
