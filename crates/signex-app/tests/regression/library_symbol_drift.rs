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

#[test]
fn a_new_pass_prunes_rows_and_names_of_unmounted_libraries_at_once() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let sheet = app.document_state.active_path.clone().unwrap();
    let ghost = format!("{}:{}", Uuid::new_v4(), Uuid::new_v4());
    let real = doc(&app).symbols[0].lib_id.clone();
    let stale = doc(&app).lib_symbols[&real].clone();
    let drift = &mut app.library.drift;
    drift.names.insert(ghost.clone(), "Gone/Part".into());
    drift.names.insert(real.clone(), "lib/TWO-UNIT".into());
    let rows = drift.drifted.entry(sheet.clone()).or_default();
    rows.insert(ghost.clone(), stale.clone());
    rows.insert(real.clone(), stale);

    // Before the worker returns: the unmounted library is already gone.
    app.library.drift.check_pending = true;
    assert!(app.take_drift_job().is_some());
    assert!(!app.library.drift.is_drifted(&sheet, &ghost));
    assert!(!app.library.drift.names.contains_key(&ghost));
    assert!(app.library.drift.is_drifted(&sheet, &real));
    assert!(app.library.drift.names.contains_key(&real));
}

#[test]
fn an_unreadable_library_symbol_keeps_the_previous_drift_row() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    assert!(is_drifted(&app));

    let path = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("two-unit.snxsym");
    std::fs::write(path, "not a symbol").unwrap();
    drop(signex_app::diagnostics::init_logging());
    check_drift(&mut app);
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Could not check library symbol")),
        "the read failure reaches the Messages panel"
    );
    assert!(
        is_drifted(&app),
        "an unreadable symbol must not clear drift"
    );
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert_eq!(app.library.drift.display_name(&lib_id), "lib/TWO-UNIT");
}

#[test]
fn a_symbol_deleted_from_the_library_clears_its_row_and_name() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    edit_library_symbol(&f, add_pin);
    check_drift(&mut app);
    assert!(is_drifted(&app));

    let path = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("two-unit.snxsym");
    std::fs::remove_file(path).unwrap();
    check_drift(&mut app);
    assert!(
        !is_drifted(&app),
        "a deleted symbol must not stay updatable"
    );
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert_eq!(app.library.drift.display_name(&lib_id), lib_id);
}
