//! #1089 — a Save As onto a path whose open is still in flight, and an
//! open result for a path a document already owns (ADR-0003 D4).
//!
//! `open_schematic_file` marks the path in `pending_opens` and returns a
//! `Task`; the document appears only when `SchematicOpenFinished` lands.
//! Until then neither `engines` nor `tabs` knows the path, so a Save As
//! that checks only those two re-keys another document onto it. The late
//! open result then pushed a second, clean tab and replaced the engine
//! with the disk content, losing every edit made after the Save As.

use signex_app::app::{EditMsg, FileMsg, Message, Signex, TabInfo, TabKind};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::path::Path;
use tempfile::TempDir;

fn symbol(uuid: uuid::Uuid) -> Symbol {
    Symbol {
        uuid,
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(0.0, 0.0),
        ..Default::default()
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    }
}

fn write_sheet(path: &Path, sheet: SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise schematic");
    std::fs::write(path, text).expect("write .snxsch");
}

/// An open, active, clean schematic tab at `path` holding one symbol.
fn open_active(app: &mut Signex, path: &Path, symbol_uuid: uuid::Uuid) -> uuid::Uuid {
    let sheet = sheet_with(vec![symbol(symbol_uuid)]);
    let sheet_uuid = sheet.uuid;
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "A".to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.to_path_buf());
    sheet_uuid
}

fn tab_count(app: &Signex, path: &Path) -> usize {
    app.document_state
        .tabs
        .iter()
        .filter(|t| t.path == path)
        .count()
}

fn panel_has(app: &Signex, needle: &str) -> bool {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .any(|entry| entry.message.contains(needle))
}

/// Open of `p` pending, Save As of the active sheet onto `p`, one edit,
/// then the open result for `p` lands. Returns the app and the edited
/// document's sheet uuid.
fn race(dir: &Path) -> (Signex, uuid::Uuid, std::path::PathBuf, std::path::PathBuf) {
    let _ = signex_app::diagnostics::init_logging();
    let a = dir.join("A.snxsch");
    let p = dir.join("P.snxsch");
    let on_disk = sheet_with(vec![symbol(uuid::Uuid::new_v4())]);
    write_sheet(&p, on_disk.clone());

    let (mut app, _boot) = Signex::new();
    let doomed = uuid::Uuid::new_v4();
    let a_uuid = open_active(&mut app, &a, doomed);

    let _ = app.update(Message::File(FileMsg::Opened(Some(p.clone()))));
    assert!(
        app.document_state.pending_opens.contains(&p),
        "fixture: the open of P must be in flight"
    );

    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::SaveAs {
            from_path: a.clone(),
            to_path: p.clone(),
        }),
    );

    // Edit the document the user was working on: delete its symbol.
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(doomed, SelectedKind::Symbol)];
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));

    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: p.clone(),
        title: "P".to_string(),
        result: Ok(Box::new(on_disk)),
    }));
    (app, a_uuid, a, p)
}

/// Passes on either guard: with the Save As refusal the edits stay on A,
/// without it the late-result drop keeps the re-keyed engine. The tests
/// below pin each guard separately.
#[test]
fn save_as_onto_a_pending_open_keeps_edits_and_one_tab() {
    let tmp = TempDir::new().expect("tempdir");
    let (app, a_uuid, a, p) = race(tmp.path());

    let edited = app
        .document_state
        .engines
        .iter()
        .find(|(_, engine)| engine.document().uuid == a_uuid)
        .map(|(path, engine)| (path.clone(), engine.document().symbols.len()))
        .expect("the edited document's engine must still exist");
    assert_eq!(
        edited.1, 0,
        "the delete made after the Save As must survive the late open result"
    );
    assert!(
        tab_count(&app, &p) <= 1 && tab_count(&app, &a) <= 1,
        "no path may have two tabs"
    );
    assert_eq!(
        app.document_state.tabs.len(),
        app.document_state
            .tabs
            .iter()
            .map(|t| &t.path)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        "tab paths must be unique"
    );
    assert!(
        app.document_state.dirty_paths.contains(&edited.0),
        "the edited document must still be flagged dirty"
    );
    let tab_dirty = app
        .document_state
        .tabs
        .iter()
        .find(|t| t.path == edited.0)
        .map(|t| t.dirty);
    assert_eq!(tab_dirty, Some(true), "its tab must still show dirty");
}

#[test]
fn save_as_onto_a_pending_open_is_refused_and_reported() {
    let tmp = TempDir::new().expect("tempdir");
    let _ = signex_app::diagnostics::init_logging();
    let a = tmp.path().join("RefusedA.snxsch");
    let p = tmp.path().join("RefusedP1089.snxsch");
    write_sheet(&p, sheet_with(Vec::new()));

    let (mut app, _boot) = Signex::new();
    let a_uuid = open_active(&mut app, &a, uuid::Uuid::new_v4());
    let _ = app.update(Message::File(FileMsg::Opened(Some(p.clone()))));
    let before = std::fs::read(&p).expect("read P");

    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::SaveAs {
            from_path: a.clone(),
            to_path: p.clone(),
        }),
    );

    assert!(
        std::fs::read(&p).expect("read P") == before,
        "nothing may be written over a path that is still opening"
    );
    let engine = app
        .document_state
        .engines
        .get(&a)
        .expect("A keeps its path");
    assert_eq!(engine.document().uuid, a_uuid);
    assert!(!app.document_state.engines.contains_key(&p));
    assert_eq!(app.document_state.tabs[0].path, a);
    assert!(
        app.document_state.pending_opens.contains(&p),
        "the pending open is untouched"
    );
    assert!(
        panel_has(&app, "RefusedP1089.snxsch"),
        "the refusal must reach the Messages panel"
    );
}

#[test]
fn a_late_open_result_for_an_owned_path_is_dropped_with_a_warning() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let owned = tmp.path().join("OwnedLate1089.snxsch");
    let (mut app, _boot) = Signex::new();
    let owner_uuid = open_active(&mut app, &owned, uuid::Uuid::new_v4());
    app.document_state.pending_opens.insert(owned.clone());

    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: owned.clone(),
        title: "late".to_string(),
        result: Ok(Box::new(sheet_with(Vec::new()))),
    }));

    assert_eq!(tab_count(&app, &owned), 1, "no second tab");
    assert_eq!(
        app.document_state
            .engines
            .get(&owned)
            .expect("engine kept")
            .document()
            .uuid,
        owner_uuid,
        "the existing engine must not be replaced"
    );
    assert!(!app.document_state.pending_opens.contains(&owned));
    assert!(
        panel_has(&app, "OwnedLate1089.snxsch"),
        "the dropped result must be logged to the Messages panel"
    );
}

fn empty_pcb_board() -> signex_types::pcb::PcbBoard {
    signex_types::pcb::PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

#[test]
fn a_late_pcb_open_result_for_an_owned_path_is_dropped_with_a_warning() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("OwnedPcb1089.snxpcb");
    let (mut app, _boot) = Signex::new();
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        path: path.clone(),
        title: "first".to_string(),
        result: Ok(Box::new(empty_pcb_board())),
    }));
    assert_eq!(tab_count(&app, &path), 1, "fixture: the first open lands");
    app.document_state.pending_opens.insert(path.clone());

    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        path: path.clone(),
        title: "late".to_string(),
        result: Ok(Box::new(empty_pcb_board())),
    }));

    assert_eq!(tab_count(&app, &path), 1, "no second PCB tab");
    assert!(!app.document_state.pending_opens.contains(&path));
    assert!(
        panel_has(&app, "OwnedPcb1089.snxpcb"),
        "the dropped result must be logged to the Messages panel"
    );
}

/// The open starts while the Save As write is running, so the refusal at
/// request time did not see it; `apply_saved_as` refuses on completion.
#[test]
fn an_open_started_during_the_save_as_write_refuses_the_rekey_and_reports_it() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let a = tmp.path().join("LateA.snxsch");
    let p = tmp.path().join("LateP1089.snxsch");
    let on_disk = sheet_with(Vec::new());
    write_sheet(&p, on_disk.clone());

    let (mut app, _boot) = Signex::new();
    let doomed = uuid::Uuid::new_v4();
    let a_uuid = open_active(&mut app, &a, doomed);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(doomed, SelectedKind::Symbol)];
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert!(
        app.document_state.dirty_paths.contains(&a),
        "fixture: A is dirty"
    );

    let write = app.update(Message::File(FileMsg::SaveAs {
        from_path: a.clone(),
        to_path: p.clone(),
    }));
    // File > Open is refused while the write runs (#1137), so mark the open
    // in flight directly: `apply_saved_as` must still refuse the re-key.
    app.document_state.pending_opens.insert(p.clone());
    crate::settle_support::settle_task(&mut app, write);

    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: p.clone(),
        title: "P".to_string(),
        result: Ok(Box::new(on_disk)),
    }));

    let engine = app
        .document_state
        .engines
        .get(&a)
        .expect("A keeps its engine");
    assert_eq!(engine.document().uuid, a_uuid);
    assert_eq!(engine.path(), Some(a.as_path()));
    assert!(app.document_state.dirty_paths.contains(&a), "A stays dirty");
    assert_eq!(tab_count(&app, &p), 1, "exactly one tab for P");
    assert!(
        panel_has(&app, "was written, but the tab was switched"),
        "the late refusal must reach the Messages panel"
    );
    assert!(
        panel_has(&app, "LateP1089.snxsch may show its previous content"),
        "the late refusal must reach the Messages panel with the stale-tab warning"
    );
}

/// A clean engine with no tab (left behind by a sheet-reference rewrite)
/// owns nothing: a later open must still read the file.
#[test]
fn a_clean_tabless_engine_does_not_block_a_later_open() {
    let tmp = TempDir::new().expect("tempdir");
    let q = tmp.path().join("Q.snxsch");
    let (mut app, _boot) = Signex::new();
    let stale = signex_engine::Engine::new(sheet_with(Vec::new())).expect("engine");
    app.document_state.engines.insert(q.clone(), stale);
    app.document_state.pending_opens.insert(q.clone());
    let fresh = sheet_with(vec![symbol(uuid::Uuid::new_v4())]);
    let fresh_uuid = fresh.uuid;

    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: q.clone(),
        title: "Q".to_string(),
        result: Ok(Box::new(fresh)),
    }));

    assert_eq!(tab_count(&app, &q), 1, "the open must not be dropped");
    assert_eq!(
        app.document_state
            .engines
            .get(&q)
            .expect("engine")
            .document()
            .uuid,
        fresh_uuid
    );
}

/// A sheet-reference rewrite loaded the parent as a dirty tab-less engine
/// while its open was in flight: the superseded result must still give the
/// user the requested tab, on the parked engine, without reading the disk.
#[test]
fn a_superseded_open_attaches_a_tab_to_a_dirty_parked_engine() {
    let tmp = TempDir::new().expect("tempdir");
    let parent = tmp.path().join("Parent.snxsch");
    let (mut app, _boot) = Signex::new();
    let parked =
        signex_engine::Engine::new(sheet_with(vec![symbol(uuid::Uuid::new_v4())])).expect("engine");
    let parked_uuid = parked.document().uuid;
    app.document_state.engines.insert(parent.clone(), parked);
    app.document_state.dirty_paths.insert(parent.clone());
    app.document_state.pending_opens.insert(parent.clone());

    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: parent.clone(),
        title: "Parent".to_string(),
        result: Ok(Box::new(sheet_with(Vec::new()))),
    }));

    assert_eq!(tab_count(&app, &parent), 1, "the requested tab must appear");
    let engine = app
        .document_state
        .engines
        .get(&parent)
        .expect("engine kept");
    assert_eq!(engine.document().uuid, parked_uuid, "engine not replaced");
    assert_eq!(engine.document().symbols.len(), 1, "edits kept");
    assert!(app.document_state.dirty_paths.contains(&parent));
}

// ---- #1137: follow-ups to #1089 ----------------------------------------

/// A directory whose path alone is longer than the ~157 bytes the Messages
/// panel keeps of an entry, so a path-first message loses its reason.
fn long_dir(root: &Path) -> std::path::PathBuf {
    let dir = root.join("a_deliberately_long_directory_name_for_the_messages_panel_cut_xx");
    std::fs::create_dir_all(&dir).expect("long dir");
    dir
}

/// The Messages entry naming `file` (unique per test: the log is global) and
/// containing `needle`.
fn panel_message_with(app: &Signex, file: &str, needle: &str) -> Option<String> {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .find(|entry| entry.message.contains(file) && entry.message.contains(needle))
        .map(|entry| entry.message.clone())
}

fn second_tab(app: &mut Signex, path: &Path) {
    app.document_state.tabs.push(TabInfo {
        title: "B".to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
}

#[test]
fn the_save_as_refusal_leads_with_its_reason_not_the_path() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = long_dir(tmp.path());
    let a = dir.join("ReasonA.snxsch");
    let p = dir.join("ReasonP1137.snxsch");
    write_sheet(&p, sheet_with(Vec::new()));
    let (mut app, _boot) = Signex::new();
    let _ = open_active(&mut app, &a, uuid::Uuid::new_v4());
    let _ = app.update(Message::File(FileMsg::Opened(Some(p.clone()))));

    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::SaveAs {
            from_path: a.clone(),
            to_path: p.clone(),
        }),
    );

    // The Messages panel keeps ~157 bytes, so the entry is only as good as
    // what it leads with: the reason must sit inside that window.
    let message = panel_message_with(&app, "ReasonP1137.snxsch", "nothing was written")
        .expect("the reason and the file name must both survive the panel's length cut");
    assert!(message.len() <= 160, "{message}");
}

#[test]
fn a_dropped_late_open_result_leads_with_its_reason_not_the_path() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = long_dir(tmp.path());
    let owned = dir.join("ReasonOwned1137.snxsch");
    let (mut app, _boot) = Signex::new();
    let _ = open_active(&mut app, &owned, uuid::Uuid::new_v4());
    app.document_state.pending_opens.insert(owned.clone());

    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: owned.clone(),
        title: "late".to_string(),
        result: Ok(Box::new(sheet_with(Vec::new()))),
    }));

    let message = panel_message_with(&app, "ReasonOwned1137.snxsch", "already owns that path");
    assert!(
        message.is_some(),
        "the reason and the file name must both survive the panel's length cut"
    );
}

/// The user only switched tabs: no tab for the target exists, so the
/// "a tab may show its previous content" line would be false.
#[test]
fn a_tab_switch_during_save_as_does_not_warn_about_a_stale_tab() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let a = tmp.path().join("SwitchA.snxsch");
    let b = tmp.path().join("SwitchB.snxsch");
    let p = tmp.path().join("SwitchP1137.snxsch");
    let (mut app, _boot) = Signex::new();
    let _ = open_active(&mut app, &a, uuid::Uuid::new_v4());
    second_tab(&mut app, &b);

    let write = app.update(Message::File(FileMsg::SaveAs {
        from_path: a.clone(),
        to_path: p.clone(),
    }));
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(b.clone());
    crate::settle_support::settle_task(&mut app, write);

    assert!(
        panel_has(&app, "was written, but the tab was switched"),
        "fixture: the re-key must be refused"
    );
    assert!(
        !panel_has(&app, "SwitchP1137.snxsch may show its previous content"),
        "no tab for the target exists, so the stale-tab warning is false"
    );
}

/// File > Open of a path while a Save As writes it would read the old
/// bytes into a clean tab; the open is refused instead.
#[test]
fn an_open_during_the_save_as_write_of_that_path_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let a = tmp.path().join("WriteA.snxsch");
    let p = tmp.path().join("WriteP1137.snxsch");
    write_sheet(&p, sheet_with(Vec::new()));
    let (mut app, _boot) = Signex::new();
    let a_uuid = open_active(&mut app, &a, uuid::Uuid::new_v4());

    let write = app.update(Message::File(FileMsg::SaveAs {
        from_path: a.clone(),
        to_path: p.clone(),
    }));
    assert!(app.document_state.document_saves.is_saving(&p), "fixture");
    let _ = app.update(Message::File(FileMsg::Opened(Some(p.clone()))));

    assert!(
        !app.document_state.pending_opens.contains(&p),
        "no read of the old bytes may start"
    );
    assert!(
        panel_has(&app, "was not opened; try again once it finishes"),
        "the refusal is reported"
    );
    crate::settle_support::settle_task(&mut app, write);

    assert_eq!(tab_count(&app, &p), 1, "only the re-keyed tab");
    assert_eq!(
        app.document_state
            .engines
            .get(&p)
            .expect("re-keyed")
            .document()
            .uuid,
        a_uuid
    );
}

#[test]
fn a_dropped_duplicate_open_activates_the_owning_tab() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let owned = tmp.path().join("FocusOwned1137.snxsch");
    let other = tmp.path().join("FocusOther1137.snxsch");
    let (mut app, _boot) = Signex::new();
    let _ = open_active(&mut app, &owned, uuid::Uuid::new_v4());
    second_tab(&mut app, &other);
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(other.clone());
    app.document_state.pending_opens.insert(owned.clone());

    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: owned.clone(),
        title: "late".to_string(),
        result: Ok(Box::new(sheet_with(Vec::new()))),
    }));

    assert_eq!(app.document_state.active_tab, 0, "the owner takes focus");
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(owned.as_path())
    );
    assert_eq!(tab_count(&app, &owned), 1);
}
