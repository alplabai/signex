//! #1845 — deleting a library symbol that component rows and placed parts
//! use warns first, and a placed part whose library symbol is gone says so.

use crate::library_place_symbol::{app_with_library, doc, fixture, place};
use crate::library_symbol_drift::{check_drift, click_update, info, pin_count, select_first};
use signex_app::app::{
    ContextMenuMsg, Message, Signex, SymbolDocument, TabContextAction, TabInfo, TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};
use signex_app::panels::PanelMsg;
use signex_library::{Symbol, SymbolFile};
use std::path::PathBuf;

pub(crate) fn container_path(f: &crate::library_place_symbol::Fixture) -> PathBuf {
    f.snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("two-unit.snxsym")
}

/// Rewrite the library's container so it also holds a sibling symbol, and
/// open it in a symbol editor tab (the SCH Library panel's Delete target).
pub(crate) fn open_container_editor(
    app: &mut Signex,
    f: &crate::library_place_symbol::Fixture,
) -> PathBuf {
    let path = container_path(f);
    let mut file = SymbolFile::from_toml_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    file.symbols.push(Symbol::empty("SIBLING"));
    std::fs::write(&path, file.to_toml_string().unwrap()).unwrap();
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "two-unit".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    path
}

pub(crate) fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

pub(crate) fn symbol_count(app: &Signex, path: &PathBuf) -> usize {
    app.document_state.symbol_editors[path].file.symbols.len()
}

#[test]
fn deleting_a_used_symbol_asks_first_and_names_the_row_and_sheet() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let reference = doc(&app).symbols[0].reference.clone();
    let path = open_container_editor(&mut app, &f);

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));

    assert_eq!(
        symbol_count(&app, &path),
        2,
        "nothing deleted before the answer"
    );
    let ctx = app.document_state.panel_ctx.symbol_editor.as_ref().unwrap();
    let warning = ctx.delete_warning.as_ref().expect("a warning is shown");
    let text = warning.lines.join("\n");
    assert!(text.contains("Component rows"), "{text}");
    assert!(text.contains("place-364.snxsch"), "{text}");
    assert!(text.contains(&reference), "{text}");

    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));
    assert_eq!(symbol_count(&app, &path), 1, "the user can still delete");
    assert!(
        app.document_state
            .panel_ctx
            .symbol_editor
            .as_ref()
            .unwrap()
            .delete_warning
            .is_none()
    );
}

#[test]
fn cancelling_the_warning_keeps_the_symbol() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    panel(&mut app, PanelMsg::SchLibraryDeleteCancelled);

    assert_eq!(symbol_count(&app, &path), 2);
    assert!(
        app.document_state
            .panel_ctx
            .symbol_editor
            .as_ref()
            .unwrap()
            .delete_warning
            .is_none()
    );
}

#[test]
fn an_unused_symbol_is_deleted_at_once() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);

    // The sibling is bound by no row and placed nowhere.
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(1));

    assert_eq!(symbol_count(&app, &path), 1);
}

#[test]
fn a_placed_part_whose_library_symbol_was_deleted_is_reported_missing() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let id = select_first(&mut app);
    let pins = pin_count(&app);

    // The library no longer holds the symbol.
    std::fs::write(
        container_path(&f),
        SymbolFile::from_symbol(Symbol::empty("OTHER"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    check_drift(&mut app);

    assert_eq!(
        info(&app, "Library Status").as_deref(),
        Some("Symbol missing")
    );
    let sheet = app.document_state.active_path.clone().unwrap();
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert!(app.library.drift.is_missing(&sheet, &lib_id));

    // The sheet keeps its embedded copy, and Update says why it does nothing.
    let _ = signex_app::diagnostics::init_logging();
    click_update(&mut app, id);
    assert!(
        logged(&[&label(&app), "is missing from its library"]) >= 1,
        "Update from library names the missing symbol"
    );
    assert_eq!(pin_count(&app), pins);
    assert_eq!(doc(&app).symbols.len(), 1);
}

fn label(app: &Signex) -> String {
    let lib_id = doc(app).symbols[0].lib_id.clone();
    app.library
        .drift
        .display_name(&lib_id, &app.library.open_libraries)
}

fn logged(needles: &[&str]) -> usize {
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| needles.iter().all(|n| e.message.contains(n)))
        .count()
}

fn remove_symbol_from_library(f: &crate::library_place_symbol::Fixture) {
    std::fs::write(
        container_path(f),
        SymbolFile::from_symbol(Symbol::empty("OTHER"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
}

#[test]
fn repeated_passes_over_a_missing_symbol_log_it_once() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    remove_symbol_from_library(&f);
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    // The line names the library; a unique name keeps the count per test.
    let marker = format!("Lib-{}", f.library_id);
    app.library.open_libraries[0].display_name = marker.clone();

    check_drift(&mut app);
    check_drift(&mut app);
    check_drift(&mut app);

    assert_eq!(
        logged(&[&marker, "library symbol missing"]),
        1,
        "the removal is reported once per change"
    );
    // The Messages panel cuts a line at 160 bytes: with the full lib_id in
    // it the closing words never arrive.
    assert_eq!(
        logged(&[
            &marker,
            "library symbol missing",
            "keeps its embedded copy."
        ]),
        1,
        "the line is short enough to arrive whole"
    );
    assert_eq!(
        logged(&[&lib_id, "Could not check library symbol"]),
        0,
        "a removal is not a failed read"
    );
}

#[test]
fn a_symbol_that_comes_back_is_no_longer_missing() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    let original = std::fs::read_to_string(container_path(&f)).unwrap();
    remove_symbol_from_library(&f);
    check_drift(&mut app);
    assert!(info(&app, "Library Status").is_some());

    std::fs::write(container_path(&f), original).unwrap();
    check_drift(&mut app);

    let sheet = app.document_state.active_path.clone().unwrap();
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert!(!app.library.drift.is_missing(&sheet, &lib_id));
    assert!(info(&app, "Library Status").is_none());
}

#[test]
fn an_unreadable_lookup_keeps_the_symbol_missing() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    remove_symbol_from_library(&f);
    check_drift(&mut app);
    assert!(info(&app, "Library Status").is_some());

    std::fs::write(container_path(&f), "not a symbol").unwrap();
    check_drift(&mut app);

    assert_eq!(
        info(&app, "Library Status").as_deref(),
        Some("Symbol missing"),
        "a failed read says nothing about the removal"
    );
}

fn warning_shown(app: &Signex) -> bool {
    app.document_state
        .panel_ctx
        .symbol_editor
        .as_ref()
        .is_some_and(|c| c.delete_warning.is_some())
}

#[test]
fn an_edit_after_the_warning_makes_it_stale_and_confirm_says_so() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(warning_shown(&app));

    // The container's history moves under the warning.
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    let snapshot = editor.snapshot();
    editor.commit_history(snapshot);
    panel(&mut app, PanelMsg::SchLibrarySelectSymbol(1));
    assert!(!warning_shown(&app), "a stale warning is not shown");

    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));
    assert_eq!(
        symbol_count(&app, &path),
        2,
        "a stale warning deletes nothing"
    );
    assert!(
        logged(&["TWO-UNIT", "changed since the warning; press Delete again"]) >= 1,
        "the refusal reaches the Messages panel"
    );
}

fn add_pin(app: &mut Signex, path: &std::path::Path) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 }),
    }));
}

#[test]
fn an_edit_at_full_history_depth_makes_the_warning_stale() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);
    for _ in 0..SymbolDocument::HISTORY_DEPTH + 5 {
        add_pin(&mut app, &path);
    }
    let depth = |app: &Signex| {
        app.document_state.symbol_editors[&path]
            .undo_snapshots
            .len()
    };
    assert_eq!(depth(&app), SymbolDocument::HISTORY_DEPTH);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(warning_shown(&app));

    // The stack stays at its cap, so a depth comparison sees no change.
    add_pin(&mut app, &path);
    assert_eq!(depth(&app), SymbolDocument::HISTORY_DEPTH);
    panel(&mut app, PanelMsg::SchLibrarySelectSymbol(1));

    assert!(!warning_shown(&app), "an edit at full depth is an edit");
}

#[test]
fn a_coalesced_field_edit_after_the_warning_makes_it_stale() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    open_container_editor(&mut app, &f);
    // The first keystroke opens the step the next one joins.
    panel(
        &mut app,
        PanelMsg::SymEditorSetSymbolName("TWO-UNIT-1".into()),
    );
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(warning_shown(&app));

    panel(
        &mut app,
        PanelMsg::SymEditorSetSymbolName("TWO-UNIT-12".into()),
    );
    panel(&mut app, PanelMsg::SchLibrarySelectSymbol(1));

    assert!(!warning_shown(&app), "a joined keystroke is an edit");
}

#[test]
fn an_undo_after_the_warning_makes_it_stale() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);
    add_pin(&mut app, &path);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(warning_shown(&app));

    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::Undo),
    }));
    panel(&mut app, PanelMsg::SchLibrarySelectSymbol(1));

    assert!(!warning_shown(&app), "an undo is an edit");
}

/// A `symbols/` folder that cannot be listed is not a removal (#1903).
#[cfg(unix)]
#[test]
fn an_unlistable_symbols_folder_does_not_mark_parts_missing() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    let folder = container_path(&f).parent().unwrap().to_path_buf();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000)).unwrap();
    let restore = || std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755));
    if std::fs::read_dir(&folder).is_ok() {
        restore().unwrap();
        eprintln!("skipped: permissions are not enforced (running as root?)");
        return;
    }

    check_drift(&mut app);
    restore().unwrap();

    let sheet = app.document_state.active_path.clone().unwrap();
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert!(!app.library.drift.is_missing(&sheet, &lib_id));
    assert_ne!(
        info(&app, "Library Status").as_deref(),
        Some("Symbol missing")
    );
}

#[test]
fn closing_the_tab_or_the_library_drops_the_warning() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(app.library.pending_symbol_delete.is_some());

    let tab = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .unwrap();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(tab),
    )));
    assert!(app.library.pending_symbol_delete.is_none(), "tab closed");

    open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(app.library.pending_symbol_delete.is_some());
    app.library.close_library(&f.snxlib);
    assert!(
        app.library.pending_symbol_delete.is_none(),
        "library closed"
    );
}

/// Rename the last header column of the fixture's `ICs` table, as a
/// hand-edited or newer library would have it: the library still opens, but
/// `read_table("ICs")` fails its schema check.
fn break_ics_table_header(f: &crate::library_place_symbol::Fixture) {
    let text = std::fs::read_to_string(&f.snxlib).unwrap();
    let mut file = signex_library::library_file::LibraryFile::parse(&text).unwrap();
    let table = file.tables.get_mut("ICs").expect("fixture table");
    let last = table.columns.len() - 1;
    table.columns[last] = format!("{}_renamed", table.columns[last]);
    std::fs::write(&f.snxlib, file.write().unwrap()).unwrap();
}

#[test]
fn a_symbol_bound_only_by_a_row_asks_first() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));

    assert_eq!(symbol_count(&app, &path), 2, "the ICs row uses it");
}

#[test]
fn a_symbol_whose_table_could_not_be_read_is_not_deleted_silently() {
    let f = fixture();
    break_ics_table_header(&f);
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));

    assert_eq!(
        symbol_count(&app, &path),
        2,
        "a table that could not be read is not proof the symbol is unused"
    );
    let ctx = app.document_state.panel_ctx.symbol_editor.as_ref().unwrap();
    let lines = ctx
        .delete_warning
        .as_ref()
        .expect("a warning")
        .lines
        .join("\n");
    assert!(
        lines.contains("Table ICs")
            && lines.contains("could not be read; its rows were not checked"),
        "{lines}"
    );
}

#[test]
fn delete_anyway_says_how_many_rows_and_sheets_now_dangle() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));

    assert!(
        logged(&[
            "TWO-UNIT deleted: 1 component row(s) and 1 open sheet(s)",
            "missing library symbol"
        ]) >= 1
    );
}

#[test]
fn restoring_the_header_and_refreshing_names_the_row_again() {
    let f = fixture();
    let healthy = std::fs::read_to_string(&f.snxlib).unwrap();
    break_ics_table_header(&f);
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    let warning = app
        .document_state
        .panel_ctx
        .symbol_editor
        .as_ref()
        .unwrap()
        .delete_warning
        .clone()
        .expect("a warning");
    assert!(warning.only_unknown, "nothing is confirmed in use");
    panel(&mut app, PanelMsg::SchLibraryDeleteCancelled);

    std::fs::write(&f.snxlib, healthy).unwrap();
    app.library
        .refresh_components(&f.snxlib)
        .expect("refresh after the repair");
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));

    let warning = app
        .document_state
        .panel_ctx
        .symbol_editor
        .as_ref()
        .unwrap()
        .delete_warning
        .clone()
        .expect("the row still uses it");
    let lines = warning.lines.join("\n");
    assert!(!lines.contains("could not be read"), "{lines}");
    assert!(lines.contains("Component rows"), "{lines}");
    assert!(!warning.only_unknown);
    assert_eq!(symbol_count(&app, &path), 2);
}

#[test]
fn a_symbol_outside_any_mounted_library_is_not_deleted_silently() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    // The library is closed (or failed to mount, or is still mounting);
    // its `.snxsym` is then opened from disk.
    app.library.close_library(&f.snxlib);
    let path = open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert_eq!(
        symbol_count(&app, &path),
        2,
        "no mounted library: its rows and the placed part were not checked, so usage is unknown"
    );
    let ctx = app.document_state.panel_ctx.symbol_editor.as_ref().unwrap();
    let warning = ctx.delete_warning.as_ref().expect("a warning");
    let lines = warning.lines.join("\n");
    assert!(lines.contains("is not inside a mounted library"), "{lines}");
    assert!(
        lines.contains("Open sheets with placed parts"),
        "the open sheet is still found by symbol uuid: {lines}"
    );
}

/// The browser marks a row's symbol cell from `dangling.marks_row`.
fn row_reads_missing(app: &Signex, f: &crate::library_place_symbol::Fixture) -> bool {
    app.library
        .library_at(&f.snxlib)
        .unwrap()
        .dangling
        .marks_row(f.row_id, signex_library::RefSlot::Symbol)
}

fn save_active(app: &mut Signex) {
    crate::settle_support::settle(app, Message::File(signex_app::app::FileMsg::Save));
}

#[test]
fn a_row_bound_to_a_deleted_symbol_reads_missing_until_the_symbol_returns() {
    let f = fixture();
    let mut app = app_with_library(&f);
    // Bootstrap queued the stock mounts; a sweep waits for them.
    app.library.pending_mounts.clear();
    let path = open_container_editor(&mut app, &f);
    assert!(!row_reads_missing(&app, &f));

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));
    save_active(&mut app);
    assert!(
        row_reads_missing(&app, &f),
        "the row keeps a broken state after the delete is saved"
    );

    // A library refresh derives the same state from the listing.
    app.library.refresh_components(&f.snxlib).unwrap();
    assert!(row_reads_missing(&app, &f), "survives a reload");

    // Undo brings the symbol back; saving it clears the state.
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .unwrap()
        .undo();
    save_active(&mut app);
    assert!(
        !row_reads_missing(&app, &f),
        "cleared when the symbol is back"
    );
}
