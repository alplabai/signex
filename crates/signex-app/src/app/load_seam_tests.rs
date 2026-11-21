//! #2326 slice 3, PR 3.0: the load seam pinned in-crate, for what the frozen
//! test adapter cannot reach (selections, undocked windows, the epoch, the
//! Messages text). The adapter half is `tests/regression/load_seam.rs`.
//! Tests named `*_today` pin current behaviour that may be a bug.

use signex_types::schematic::{SelectedItem, SelectedKind, Symbol};

use super::*;
use crate::app::handlers::menu::export::tests::app_workspace;

const DIR: &str = "/w/proj";

fn sheet_path(name: &str) -> PathBuf {
    PathBuf::from(DIR).join(name)
}

/// A file name no other test logs, so a Messages count cannot be moved by
/// tests running in parallel.
fn unique(stem: &str) -> String {
    format!("{stem}_{}.snxsch", uuid::Uuid::now_v7().simple())
}

fn sheet(paper: &str) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: paper.to_string(),
        ..Default::default()
    }
}

fn sheet_with_symbol(symbol: uuid::Uuid) -> SchematicSheet {
    let mut sheet = sheet("A4");
    sheet.symbols.push(Symbol {
        uuid: symbol,
        ..Symbol::default()
    });
    sheet
}

/// An app with the project at `DIR` open and `name` opened from disk.
fn opened(name: &str, sheet: SchematicSheet) -> (Signex, PathBuf) {
    let mut app = app_workspace(DIR, &[name]);
    let path = sheet_path(name);
    app.open_schematic_tab(path.clone(), name.to_string(), sheet);
    (app, path)
}

fn selection(uuid: uuid::Uuid) -> Vec<SelectedItem> {
    vec![SelectedItem::new(uuid, SelectedKind::Symbol)]
}

/// An undocked window showing `path`, registered the way `UndockTab` does.
fn undocked_window(app: &mut Signex, path: &std::path::Path) -> iced::window::Id {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        super::state::WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "w".into(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    window
}

fn edit_of(uuid: uuid::Uuid) -> TextEditState {
    TextEditState {
        uuid,
        kind: SelectedKind::Symbol,
        text: "R1".into(),
        original_text: "R1".into(),
        world_x: 0.0,
        world_y: 0.0,
        sheet: None,
        placement: None,
    }
}

/// The Messages row holding `needle` reads as `full` does in the panel:
/// whole when it fits, else its first 157 bytes and "...". Exactly one such
/// row was added since `before`.
fn assert_listed_once(needle: &str, before: usize, full: &str) {
    assert_eq!(
        crate::diagnostics::logged_count(needle),
        before + 1,
        "{full}"
    );
    let row = crate::diagnostics::recent_entries()
        .into_iter()
        .find(|entry| entry.message.contains(needle))
        .expect("the row is listed")
        .message;
    let shown = if full.len() <= 160 {
        full.to_string()
    } else {
        format!("{}...", &full[..157])
    };
    assert_eq!(row, shown);
}

/// T3: inside `open_schematic_tab`, panel_ctx is built before the dirty mark
/// of a repaired sheet.
#[test]
fn a_panel_refresh_on_open_runs_before_the_repair_dirty_mark_today() {
    let repaired = crate::app::sheet_repairs::tests::repaired_sheet(false);
    let (app, path) = opened("t3.snxsch", repaired);

    assert!(app.document_state.is_document_dirty(&path));
    let project = &app.document_state.panel_ctx.projects[0];
    assert!(
        project.project_file_open,
        "fixture: the tree row is the sheet"
    );
    assert!(
        !project.project_file_dirty,
        "the tree was built before the dirty mark"
    );
}

/// T4: a History restore of the sheet in view installs it at its save point
/// and drops the selection in the main window and in every undocked one.
#[test]
fn a_restore_of_the_active_sheet_installs_it_clean_and_drops_the_selection_in_every_window() {
    let picked = uuid::Uuid::now_v7();
    let (mut app, path) = opened("t4.snxsch", sheet_with_symbol(picked));
    let window = undocked_window(&mut app, &path);
    app.interaction_state
        .canvases
        .get_mut(&window)
        .unwrap()
        .selected = selection(picked);
    app.interaction_state.active_canvas_mut().selected = selection(picked);

    let idx = app.document_state.active_tab;
    app.install_reloaded_schematic(idx, &path, sheet("A3"), false, None);

    let engine = &app.document_state.engines[&path];
    assert_eq!(engine.document().paper_size, "A3", "installed");
    assert!(engine.is_at_save_point(), "installed clean");
    assert!(app.interaction_state.active_canvas().selected.is_empty());
    assert!(app.interaction_state.canvases[&window].selected.is_empty());
}

/// T6: a restore of a sheet not in view drops the inline edit only when it
/// targets an object of that sheet's old engine.
#[test]
fn a_non_active_restore_drops_an_inline_edit_only_if_it_targets_that_sheet() {
    let (in_view, old_only) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let (mut app, _a) = opened("t6a.snxsch", sheet_with_symbol(in_view));
    let b = sheet_path("t6b.snxsch");
    let engine =
        signex_engine::Engine::new_with_path(sheet_with_symbol(old_only), Some(b.clone())).unwrap();
    app.document_state.engines.insert(b.clone(), engine);

    app.interaction_state.editing_text = Some(edit_of(in_view));
    assert!(app.install_schematic_at(&b, sheet("A4")));
    assert!(
        app.interaction_state.editing_text.is_some(),
        "an edit on another sheet is kept"
    );

    let engine =
        signex_engine::Engine::new_with_path(sheet_with_symbol(old_only), Some(b.clone())).unwrap();
    app.document_state.engines.insert(b.clone(), engine);
    app.interaction_state.editing_text = Some(edit_of(old_only));
    assert!(app.install_schematic_at(&b, sheet("A4")));
    assert!(
        app.interaction_state.editing_text.is_none(),
        "an edit on an object only the old engine holds is dropped"
    );
}

/// T7 (F5): installing a sheet not in view leaves the documents epoch alone;
/// installing the active one moves it.
#[test]
fn a_non_active_restore_leaves_the_documents_epoch_alone_and_an_active_one_moves_it_today() {
    let (mut app, a) = opened("t7a.snxsch", sheet("A4"));
    let b = sheet_path("t7b.snxsch");
    let engine = signex_engine::Engine::new_with_path(sheet("A4"), Some(b.clone())).unwrap();
    app.document_state.engines.insert(b.clone(), engine);

    let before = app.document_state.documents_epoch;
    assert!(app.install_schematic_at(&b, sheet("A3")));
    assert_eq!(app.document_state.documents_epoch, before, "non-active");

    let fresh = signex_engine::Engine::new_with_path(sheet("A3"), Some(a.clone()));
    assert!(app.install_active_engine(a, fresh));
    assert_eq!(app.document_state.documents_epoch, before + 1, "active");
}

/// T8: the read-only notice of an open, as listed.
#[test]
fn opening_a_read_only_sheet_lists_its_notice() {
    let name = unique("t8ro");
    let mut ro = sheet("A4");
    ro.unknown.newer_minor_source = Some("snxsch/1.7".into());
    let before = crate::diagnostics::logged_count(&name);
    let (_app, path) = opened(&name, ro);
    let full = format!(
        "{} was written by a newer Signex (snxsch/1.7); it is read-only in this build -- saving is refused",
        path.display()
    );
    assert_listed_once(&name, before, &full);
}

/// T8: the bare pin tap report of an open, as listed.
#[test]
fn opening_a_sheet_with_a_bare_pin_tap_lists_the_report() {
    let name = unique("t8tap");
    let mut tapped = crate::app::pin_junction_repair::tests::sheet_with_bare_tap();
    // A known size, so the size report does not name the file too.
    tapped.paper_size = "A4".into();
    let before = crate::diagnostics::logged_count(&name);
    let (_app, _path) = opened(&name, tapped);
    let full = format!(
        "{name}: 1 pin touches a wire without a junction; Repair Pin Junctions adds the dots"
    );
    assert_listed_once(&name, before, &full);
}

/// T8: the repair report of an open, as listed.
#[test]
fn opening_a_repaired_sheet_lists_the_repair() {
    let name = unique("t8rep");
    let repaired = crate::app::sheet_repairs::tests::repaired_sheet(false);
    let described = repaired.unknown.load_repairs[0].describe();
    let before = crate::diagnostics::logged_count(&name);
    let (_app, _path) = opened(&name, repaired);
    let full = format!("{name} repaired on load: {described}");
    assert_listed_once(&name, before, &full);
}

/// T8: the unusable zone hole report of a board open, as listed.
#[test]
fn opening_a_board_with_a_malformed_zone_hole_lists_the_report() {
    let name = unique("t8zone").replace(".snxsch", ".snxpcb");
    let path = sheet_path(&name);
    let text = "format = \"snxpcb/1\"\npcb_id = \"00000000-0000-0000-0000-000000000009\"\n";
    let mut board = signex_types::format::SnxPcb::parse(text)
        .expect("parses")
        .board;
    board.zones = vec![
        serde_json::from_value(serde_json::json!({
            "uuid": uuid::Uuid::now_v7(), "net": 1, "net_name": "GND", "layer": "TopCopper",
            "outline": [{"x": 0.0, "y": 0.0}, {"x": 9.0, "y": 0.0}, {"x": 9.0, "y": 9.0}],
            "holes": [[{"x": 1.0, "y": 1.0}, {"x": 2.0, "y": 1.0}]]
        }))
        .expect("a zone"),
    ];
    let before = crate::diagnostics::logged_count(&name);
    let mut app = app_workspace(DIR, &[]);
    app.open_pcb_tab(path.clone(), name.clone(), board);
    let full = format!(
        "{}: zone GND on TopCopper: hole 0 has fewer than 3 vertices; drawn as solid copper \
         (a hole must be a simple ring strictly inside its outline, apart from the other holes)",
        path.display()
    );
    assert_listed_once(&name, before, &full);
}

/// T8: the failed active install notice, as listed.
#[test]
fn a_failed_active_install_lists_its_notice() {
    let name = unique("t8fail");
    let (mut app, path) = opened(&name, sheet("A4"));
    let before = crate::diagnostics::logged_count(&name);
    let failed = Err(signex_engine::EngineError::MissingPath);
    assert!(!app.install_active_engine(path.clone(), failed));
    let full = format!(
        "{} could not be loaded into the editor (engine document path is not set); \
         the sheet already open was kept",
        path.display()
    );
    assert_listed_once(&name, before, &full);
}

/// T8: the placeholder notice for a symbol whose library entry is missing.
#[test]
fn opening_a_sheet_with_a_missing_library_entry_lists_the_placeholder_notice() {
    let lib_id = format!("Gone:{}", uuid::Uuid::now_v7().simple());
    let mut missing = sheet("A4");
    missing.symbols.push(Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: lib_id.clone(),
        ..Symbol::default()
    });
    let before = crate::diagnostics::logged_count(&lib_id);
    let (_app, _path) = opened("t8lib.snxsch", missing);
    let full = format!(
        "Library entry for symbol '{lib_id}' is missing on this sheet; it is drawn as a crossed-out placeholder box"
    );
    assert_listed_once(&lib_id, before, &full);
}

/// T9 (in-crate half): the Save As re-key clears the old path's dirty mark
/// and moves an undocked window on the sheet to the new file.
#[test]
fn save_as_rekeys_the_old_dirty_mark_and_the_undocked_window() {
    let (mut app, from) = opened("t9.snxsch", sheet("A4"));
    app.document_state.mark_document_dirty(&from);
    let window = undocked_window(&mut app, &from);
    let to = sheet_path("t9copy.snxsch");
    let revision = app.document_state.engines[&from].revision();

    let renamed = app.rekey_active_schematic(to.clone(), |engine| {
        assert_eq!(engine.revision(), revision);
        engine.mark_saved();
        false
    });

    assert!(renamed);
    assert!(
        !app.document_state.is_document_dirty(&from),
        "old mark gone"
    );
    assert!(
        !app.document_state.is_document_dirty(&to),
        "the copy is saved"
    );
    assert!(app.document_state.engines.contains_key(&to));
    assert!(!app.document_state.engines.contains_key(&from));
    assert_eq!(app.active_tab_path(), Some(to.clone()));
    assert!(matches!(
        &app.ui_state.windows[&window],
        super::state::WindowKind::UndockedTab { path, .. } if *path == to
    ));
}

/// T10: a tab switch to another sheet drops the selection and the drawing
/// buffers; a refresh of the same sheet keeps them.
#[test]
fn switching_sheets_clears_selection_drawing_and_placement_only_when_the_sheet_changes() {
    let picked = uuid::Uuid::now_v7();
    let (mut app, _a) = opened("t10a.snxsch", sheet("A4"));
    app.open_schematic_tab(
        sheet_path("t10b.snxsch"),
        "b".into(),
        sheet_with_symbol(picked),
    );
    let point = signex_types::schematic::Point::new(1.0, 2.0);
    app.interaction_state.active_canvas_mut().selected = selection(picked);
    app.interaction_state.polyline_points = vec![point];

    app.sync_visible_document_from_active_tab();
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        selection(picked),
        "same sheet"
    );
    assert_eq!(app.interaction_state.polyline_points, vec![point]);

    app.document_state.active_tab = 0;
    app.sync_visible_document_from_active_tab();
    assert!(app.interaction_state.active_canvas().selected.is_empty());
    assert!(app.interaction_state.polyline_points.is_empty());
}

/// T11: reattaching a parked dirty sheet makes its engine, with its edits,
/// the active one, still dirty.
#[test]
fn reattaching_a_parked_dirty_sheet_keeps_its_edits_and_dirty_mark() {
    let (mut app, path) = opened("t11.snxsch", sheet("A4"));
    let junction = signex_types::schematic::Junction {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    };
    app.document_state
        .engines
        .get_mut(&path)
        .unwrap()
        .execute(signex_engine::Command::PlaceJunction { junction })
        .unwrap();
    app.document_state.mark_document_dirty(&path);
    // Closing a dirty tab parks its engine; the close starts no work here.
    let _closing = app.close_tab_parking_dirty(app.document_state.active_tab);
    assert!(app.document_state.tabs.is_empty(), "fixture: closed");
    assert!(app.document_state.engines.contains_key(&path), "parked");

    app.attach_parked_schematic_tab(path.clone(), "t11".into());

    assert_eq!(app.document_state.active_path, Some(path.clone()));
    let active = app.active_schematic().expect("the parked engine is active");
    assert_eq!(active.junctions.len(), 1, "the edit survives");
    assert!(app.document_state.is_document_dirty(&path));
}

/// Restore the recovery copy `copy` into the open sheet at `path`.
fn restore_recovery_copy(app: &mut Signex, path: &std::path::Path, copy: SchematicSheet) {
    use crate::autosave::{AutosaveMsg, RecoveryOffer, RestoreLoaded, RestoredDocument};
    let loaded = RestoreLoaded {
        path: path.to_path_buf(),
        offer: RecoveryOffer {
            path: path.to_path_buf(),
            age_secs: 0,
            older_than_file: false,
        },
        result: Ok(RestoredDocument::Schematic(Box::new(copy))),
    };
    // The restore returns no work for a schematic; nothing to settle.
    let _restored = app.update(Message::Autosave(AutosaveMsg::RestoreLoaded(loaded)));
}

/// T12: a recovery restore brings back content, not identity: the open
/// sheet keeps its document id.
#[test]
fn a_recovery_restore_keeps_the_live_document_id() {
    let live = sheet("A4");
    let live_id = live.uuid;
    let (mut app, path) = opened("t12.snxsch", live);

    restore_recovery_copy(&mut app, &path, sheet("A3"));

    let document = app.document_state.engines[&path].document();
    assert_eq!(document.paper_size, "A3", "the copy was restored");
    assert_eq!(document.uuid, live_id);
    assert!(app.document_state.is_document_dirty(&path));
}

/// A recovery restore installs a new engine without clearing the selection,
/// so it can name objects the restored sheet lacks (the #102 class).
#[test]
fn a_recovery_restore_keeps_the_old_selection_today() {
    let picked = uuid::Uuid::now_v7();
    let (mut app, path) = opened("t12sel.snxsch", sheet_with_symbol(picked));
    app.interaction_state.active_canvas_mut().selected = selection(picked);

    restore_recovery_copy(&mut app, &path, sheet("A3"));

    assert!(
        app.document_state.engines[&path]
            .document()
            .symbols
            .is_empty()
    );
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        selection(picked)
    );
}

/// T13: opening a board holds autosave for it and queues its recovery check.
#[test]
fn opening_a_board_holds_autosave_and_queues_the_recovery_check() {
    let path = sheet_path("t13.snxpcb");
    let text = "format = \"snxpcb/1\"\npcb_id = \"00000000-0000-0000-0000-000000000009\"\n";
    let board = signex_types::format::SnxPcb::parse(text)
        .expect("parses")
        .board;
    let mut app = app_workspace(DIR, &[]);

    app.open_pcb_tab(path.clone(), "t13".into(), board);

    assert!(
        app.document_state
            .autosave
            .checking_paths()
            .any(|checking| *checking == path),
        "autosave is held"
    );
    assert!(app.document_state.recovery_check_due.contains(&path));
}
