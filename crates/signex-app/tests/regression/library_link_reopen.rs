//! #1338 — a placed symbol's library link (`library_id`, `row_id`,
//! `library_version`) is saved in the `.snxsch`, so reopening the sheet runs
//! the open-time library-update scan for real. These tests save a linked
//! sheet and reopen it through the open-completion path, with the library
//! mounted and drifted, mounted and unchanged, and not mounted.

use crate::library_place_symbol::{Fixture, app_with_library, doc, fixture, place, team_fixture};
use crate::library_symbol_drift::{add_pin, edit_library_symbol};
use signex_app::app::{FileMsg, Message, Signex};
use signex_types::schematic::SchematicSheet;
use std::path::{Path, PathBuf};

const PLACED: usize = 3;
const STALE: &str = "0.0.1";

/// A sheet with `PLACED` symbols from the fixture library, each pinned to
/// the version `STALE`, written to `.snxsch` text and parsed back.
fn saved_linked_sheet(f: &Fixture) -> SchematicSheet {
    let mut app = app_with_library(f);
    for _ in 0..PLACED {
        place(&mut app, f);
    }
    let mut sheet = doc(&app).clone();
    assert_eq!(sheet.symbols.len(), PLACED);
    for symbol in &mut sheet.symbols {
        symbol.library_version = STALE.into();
    }
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("write sheet");
    signex_types::format::SnxSchematic::parse(&text)
        .expect("parse sheet")
        .sheet
}

/// Pin count of the first placed symbol's embedded copy.
fn pins(sheet: &SchematicSheet) -> usize {
    sheet.lib_symbols[&sheet.symbols[0].lib_id].pins.len()
}

fn reopen(app: &mut Signex, sheet: SchematicSheet) -> PathBuf {
    let path = PathBuf::from("reopen-1338.snxsch");
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: path.clone(),
        title: "reopen-1338".into(),
        result: Ok(Box::new(sheet)),
    }));
    path
}

fn app_with_mounted(f: &Fixture) -> Signex {
    let (mut app, _t) = Signex::new();
    app.library
        .open_library(f.snxlib.clone())
        .expect("mount library");
    app
}

fn warnings_containing(needle: &str) -> usize {
    let _ = signex_app::diagnostics::init_logging();
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(needle))
        .count()
}

fn not_mounted_warnings(f: &Fixture) -> usize {
    let id = f.library_id.to_string();
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains("library not mounted") && e.message.contains(&id))
        .count()
}

#[test]
fn the_link_survives_save_and_reopen() {
    let f = fixture();
    let sheet = saved_linked_sheet(&f);
    for symbol in &sheet.symbols {
        assert_eq!(symbol.library_id, Some(f.library_id));
        assert_eq!(symbol.row_id, Some(f.row_id));
        assert_eq!(symbol.library_version, STALE);
    }
}

#[test]
fn a_mounted_drifted_library_updates_the_personal_sheet_once_and_marks_it_dirty() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let sheet = saved_linked_sheet(&f);
    let pins_before = pins(&sheet);
    edit_library_symbol(&f, add_pin);

    let mut app = app_with_mounted(&f);
    let path = reopen(&mut app, sheet);

    let opened = app.document_state.engines[&path].document();
    // The documented Personal-mode auto-update: the embedded copy is
    // replaced, every instance is re-stamped, the sheet is dirty, and no
    // modal is raised.
    assert_eq!(pins(opened), pins_before + 1);
    assert_eq!(opened.symbols.len(), PLACED);
    for symbol in &opened.symbols {
        assert_ne!(symbol.library_version, STALE);
        assert_eq!(symbol.library_id, Some(f.library_id), "link kept");
        assert_eq!(symbol.row_id, Some(f.row_id), "link kept");
    }
    assert!(app.document_state.is_document_dirty(&path));
    assert!(app.library.library_updates.is_none());
    assert_eq!(not_mounted_warnings(&f), 0);
}

#[test]
fn a_mounted_library_that_has_not_drifted_changes_nothing() {
    let f = fixture();
    let mut sheet = saved_linked_sheet(&f);
    // Pin the symbols at the version the library really has.
    let current = {
        let mut app = app_with_library(&f);
        place(&mut app, &f);
        doc(&app).symbols[0].library_version.clone()
    };
    for symbol in &mut sheet.symbols {
        symbol.library_version = current.clone();
    }
    let before = sheet.clone();

    let mut app = app_with_mounted(&f);
    let path = reopen(&mut app, sheet);

    let opened = app.document_state.engines[&path].document();
    assert_eq!(pins(opened), pins(&before), "embedded copy untouched");
    assert_eq!(opened.symbols.len(), PLACED);
    for (a, b) in opened.symbols.iter().zip(&before.symbols) {
        assert_eq!(a.library_version, b.library_version);
    }
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(app.library.library_updates.is_none());
}

#[test]
fn an_unmounted_library_warns_once_and_leaves_the_sheet_alone() {
    let f = fixture();
    let sheet = saved_linked_sheet(&f);
    let before = sheet.clone();
    let _ = signex_app::diagnostics::init_logging();

    let (mut app, _t) = Signex::new();
    let path = reopen(&mut app, sheet);

    assert_eq!(
        not_mounted_warnings(&f),
        1,
        "{PLACED} symbols from one unmounted library log one line, not one each"
    );
    let opened = app.document_state.engines[&path].document();
    assert_eq!(pins(opened), pins(&before), "embedded copy untouched");
    for symbol in &opened.symbols {
        assert_eq!(symbol.library_version, STALE);
        assert_eq!(symbol.library_id, Some(f.library_id), "link kept");
    }
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(app.library.library_updates.is_none());
}

#[test]
fn a_malformed_link_id_in_the_file_opens_the_sheet_unlinked_and_is_reported() {
    let f = fixture();
    let sheet = saved_linked_sheet(&f);
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("write sheet");
    let text = text.replace(&f.library_id.to_string(), "not-a-uuid");
    assert!(text.contains("not-a-uuid"));
    let _ = signex_app::diagnostics::init_logging();
    let before = warnings_containing("library_id = \"not-a-uuid\"");

    let sheet = signex_types::format::SnxSchematic::parse(&text)
        .expect("a malformed link id must not fail the sheet")
        .sheet;
    let (mut app, _t) = Signex::new();
    let path = reopen(&mut app, sheet);

    let opened = app.document_state.engines[&path].document();
    assert_eq!(opened.symbols.len(), PLACED);
    assert!(opened.symbols.iter().all(|s| s.library_id.is_none()));
    assert!(warnings_containing("library_id = \"not-a-uuid\"") > before);
}

// ── #1449 — a library that mounts after the sheet opened ──────────────

fn reopen_at(app: &mut Signex, sheet: SchematicSheet, name: &str) -> PathBuf {
    let path = PathBuf::from(format!("{name}.snxsch"));
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: path.clone(),
        title: name.into(),
        result: Ok(Box::new(sheet)),
    }));
    path
}

/// Land a background mount of `f`'s library the way `spawn_pending_mounts`
/// does: record the request, prepare off-thread, deliver `MountFinished`.
fn finish_mount(app: &mut Signex, f: &Fixture) {
    use signex_app::library::LibraryMessage;
    use signex_app::library::mount::{MountIntent, prepare_mount_off_thread};
    app.library.request_mount(&f.snxlib, MountIntent::Silent);
    let prepared = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(prepare_mount_off_thread(f.snxlib.clone()));
    let _ = app.update(Message::Library(LibraryMessage::MountFinished {
        path: f.snxlib.clone(),
        prepared,
    }));
}

fn is_dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path)
}

#[test]
fn a_late_mount_lands_the_update_on_the_open_personal_sheet() {
    let f = fixture();
    let sheet = saved_linked_sheet(&f);
    let pins_before = pins(&sheet);
    edit_library_symbol(&f, add_pin);
    let _ = signex_app::diagnostics::init_logging();

    let (mut app, _t) = Signex::new();
    let path = reopen(&mut app, sheet);
    assert_eq!(not_mounted_warnings(&f), 1);
    assert_eq!(
        pins(app.document_state.engines[&path].document()),
        pins_before
    );
    assert!(!is_dirty(&app, &path));

    finish_mount(&mut app, &f);

    let opened = app.document_state.engines[&path].document();
    assert_eq!(pins(opened), pins_before + 1, "update landed after mount");
    for symbol in &opened.symbols {
        assert_ne!(symbol.library_version, STALE);
        assert_eq!(symbol.library_id, Some(f.library_id), "link kept");
    }
    assert!(is_dirty(&app, &path));
    assert!(app.library.library_updates.is_none());
    assert_eq!(not_mounted_warnings(&f), 1, "the mount adds no warning");
}

#[test]
fn an_unrelated_mount_does_not_rescan_the_sheet() {
    let f = fixture();
    let other = fixture();
    let sheet = saved_linked_sheet(&f);
    let pins_before = pins(&sheet);
    let _ = signex_app::diagnostics::init_logging();

    let (mut app, _t) = Signex::new();
    let path = reopen(&mut app, sheet);
    finish_mount(&mut app, &f);
    assert_eq!(
        pins(app.document_state.engines[&path].document()),
        pins_before,
        "no drift yet, nothing to land"
    );

    // The library drifts after the first scan; mounting a different
    // library must not pick that up for this sheet.
    edit_library_symbol(&f, add_pin);
    finish_mount(&mut app, &other);

    let opened = app.document_state.engines[&path].document();
    assert_eq!(
        pins(opened),
        pins_before,
        "no re-scan for an unrelated mount"
    );
    assert_eq!(not_mounted_warnings(&f), 1);
}

#[test]
fn a_late_mount_leaves_a_sheet_without_matching_links_alone() {
    let f = fixture();
    let linked = saved_linked_sheet(&f);
    let mut unlinked = linked.clone();
    for symbol in &mut unlinked.symbols {
        symbol.library_id = None;
        symbol.row_id = None;
    }
    let before = unlinked.clone();
    edit_library_symbol(&f, add_pin);

    let (mut app, _t) = Signex::new();
    let plain = reopen_at(&mut app, unlinked, "plain-1449");
    finish_mount(&mut app, &f);

    let opened = app.document_state.engines[&plain].document();
    assert_eq!(pins(opened), pins(&before));
    for (a, b) in opened.symbols.iter().zip(&before.symbols) {
        assert_eq!(a.library_version, b.library_version);
    }
    assert!(!is_dirty(&app, &plain));
    assert!(app.library.library_updates.is_none());
}

// ── #1449 — Team mode: the Library Updates modal after a late mount ───

fn modal_libraries(app: &Signex) -> Vec<uuid::Uuid> {
    app.library
        .library_updates
        .as_ref()
        .map(|m| m.entries.iter().map(|e| e.library_id).collect())
        .unwrap_or_default()
}

#[test]
fn a_late_mount_opens_the_team_updates_modal_with_that_librarys_entries() {
    let f = team_fixture("teama");
    let sheet = saved_linked_sheet(&f);

    let (mut app, _t) = Signex::new();
    let path = reopen(&mut app, sheet);
    assert!(app.library.library_updates.is_none(), "nothing to show yet");

    finish_mount(&mut app, &f);

    let modal = app.library.library_updates.as_ref().expect("modal opens");
    assert_eq!(modal.schematic_path, path);
    assert_eq!(modal.entries.len(), PLACED);
    assert!(modal.entries.iter().all(|e| e.library_id == f.library_id));
    // Team mode never rewrites the sheet behind the user's back.
    assert!(!is_dirty(&app, &path));
}

#[test]
fn a_second_late_mount_adds_to_the_open_team_modal_without_dropping_entries() {
    let a = team_fixture("teama");
    let b = team_fixture("teamb");
    let mut sheet = saved_linked_sheet(&a);
    let mut from_b = saved_linked_sheet(&b);
    for (n, symbol) in from_b.symbols.iter_mut().enumerate() {
        symbol.reference = format!("U{}", 100 + n);
    }
    sheet.lib_symbols.extend(
        from_b
            .lib_symbols
            .iter()
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    sheet.symbols.extend(from_b.symbols);

    let (mut app, _t) = Signex::new();
    let _path = reopen(&mut app, sheet);
    finish_mount(&mut app, &a);
    assert_eq!(modal_libraries(&app), vec![a.library_id; PLACED]);

    finish_mount(&mut app, &b);

    let libs = modal_libraries(&app);
    assert_eq!(libs.len(), 2 * PLACED);
    assert_eq!(libs.iter().filter(|l| **l == a.library_id).count(), PLACED);
    assert_eq!(libs.iter().filter(|l| **l == b.library_id).count(), PLACED);
}

#[test]
fn a_later_mount_keeps_the_users_checkbox_choices_in_the_open_team_modal() {
    let a = team_fixture("teama");
    let b = team_fixture("teamb");
    let mut sheet = saved_linked_sheet(&a);
    let mut from_b = saved_linked_sheet(&b);
    for (n, symbol) in from_b.symbols.iter_mut().enumerate() {
        symbol.reference = format!("U{}", 100 + n);
    }
    sheet.lib_symbols.extend(
        from_b
            .lib_symbols
            .iter()
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    sheet.symbols.extend(from_b.symbols);

    let (mut app, _t) = Signex::new();
    let _path = reopen(&mut app, sheet);
    finish_mount(&mut app, &a);
    let modal = app.library.library_updates.as_mut().expect("modal");
    let toggled = modal.entries[0].symbol_uuid;
    let default = modal.entries[0].selected;
    modal.toggle(toggled);
    let chosen = modal
        .entries
        .iter()
        .find(|e| e.symbol_uuid == toggled)
        .expect("entry")
        .selected;
    assert_ne!(chosen, default, "the user changed the checkbox");

    finish_mount(&mut app, &b);

    let modal = app.library.library_updates.as_ref().expect("modal");
    let entry = modal
        .entries
        .iter()
        .find(|e| e.symbol_uuid == toggled)
        .expect("entry kept");
    assert_eq!(entry.selected, chosen, "choice survives the second mount");
}

#[test]
fn a_shared_lib_id_with_an_unmounted_sibling_library_blocks_the_auto_apply() {
    let a = fixture();
    let b = fixture();
    let mut sheet = saved_linked_sheet(&a);
    // Instances placed from A's copy but linked to B (a row in B whose
    // symbol lives in A): same `lib_id`, different library.
    let mut from_b = sheet.symbols.clone();
    for (n, symbol) in from_b.iter_mut().enumerate() {
        symbol.uuid = uuid::Uuid::now_v7();
        symbol.reference = format!("U{}", 100 + n);
        symbol.library_id = Some(b.library_id);
        symbol.row_id = Some(b.row_id);
    }
    sheet.symbols.extend(from_b);
    let pins_before = pins(&sheet);
    edit_library_symbol(&a, add_pin);

    let (mut app, _t) = Signex::new();
    let path = reopen(&mut app, sheet);
    finish_mount(&mut app, &a);

    // B is not mounted, so its instances cannot be checked: the shared
    // embedded copy must not be replaced underneath them.
    let opened = app.document_state.engines[&path].document();
    assert_eq!(pins(opened), pins_before);
    assert!(opened.symbols.iter().all(|s| s.library_version == STALE));
    assert!(!is_dirty(&app, &path));
}

#[test]
fn a_late_mount_after_an_edit_lands_as_one_undo_step() {
    use signex_app::canvas::CanvasEvent;
    use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};
    let f = fixture();
    let mut sheet = saved_linked_sheet(&f);
    let wire = uuid::Uuid::new_v4();
    sheet.wires.push(Wire {
        unknown_columns: Default::default(),
        uuid: wire,
        start: Point::new(10.0, 10.0),
        end: Point::new(30.0, 10.0),
        stroke_width: 0.0,
    });
    let pins_before = pins(&sheet);
    edit_library_symbol(&f, add_pin);

    let (mut app, _t) = Signex::new();
    app.ui_state.snap_enabled = false;
    let path = reopen(&mut app, sheet);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(wire, SelectedKind::Wire)];
    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 2.54,
        dy: 0.0,
        connections: Default::default(),
    }));
    let wire_x = |app: &Signex| {
        app.document_state.engines[&path]
            .document()
            .wires
            .iter()
            .find(|w| w.uuid == wire)
            .expect("wire")
            .start
            .x
    };
    let moved = wire_x(&app);
    assert!((moved - 12.54).abs() < 1e-6, "the edit happened");

    finish_mount(&mut app, &f);

    let opened = app.document_state.engines[&path].document();
    assert_eq!(
        pins(opened),
        pins_before + 1,
        "update landed after the edit"
    );
    assert!(is_dirty(&app, &path));

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    let opened = app.document_state.engines[&path].document();
    assert_eq!(pins(opened), pins_before, "one undo reverts the update");
    assert!((wire_x(&app) - moved).abs() < 1e-9, "the wire edit remains");
}

#[test]
fn a_late_team_mount_queues_every_open_sheets_modal() {
    let f = team_fixture("teamq");
    let (mut app, _t) = Signex::new();
    let a = reopen_at(&mut app, saved_linked_sheet(&f), "queue-a");
    let b = reopen_at(&mut app, saved_linked_sheet(&f), "queue-b");

    finish_mount(&mut app, &f);

    let first = app
        .library
        .library_updates
        .as_ref()
        .expect("a modal")
        .clone();
    assert_eq!(first.entries.len(), PLACED);
    assert_eq!(
        app.library.queued_library_updates.len(),
        1,
        "other sheet waits"
    );
    let second_path = app.library.queued_library_updates[0].schematic_path.clone();
    assert_ne!(first.schematic_path, second_path);
    assert!([&a, &b].contains(&&first.schematic_path));
    assert!([&a, &b].contains(&&second_path));

    let _ = app.update(Message::Library(
        signex_app::library::LibraryMessage::LibraryUpdatesSkipAll,
    ));

    let next = app.library.library_updates.as_ref().expect("next modal");
    assert_eq!(next.schematic_path, second_path);
    assert_eq!(next.entries.len(), PLACED, "its entries were not lost");
    assert!(app.library.queued_library_updates.is_empty());
}
