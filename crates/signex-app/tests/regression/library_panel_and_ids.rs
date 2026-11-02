//! #1895 — the Library panel's ▸ expands a library to list its
//! components; one click highlights a component, a double click opens it.
//! #1894 — a copy of a mounted library (same `library_id`, other path) is
//! refused instead of mounting beside it, so lookups by id have one answer.

use std::path::{Path, PathBuf};

use signex_app::app::{Message, Signex, TabKind};
use signex_app::library::LibraryMessage;
use signex_app::library::messages::PickerMsg;
use signex_app::library::state::ComponentsPanelRowKey;
use tempfile::TempDir;

use crate::library_place_symbol::run;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("entry");
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), &dest).expect("copy");
        }
    }
}

fn stock_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library")
}

fn settle(app: &mut Signex, message: Message) {
    let mut pending = vec![message];
    while let Some(next) = pending.pop() {
        let task = app.update(next);
        pending.extend(run(task));
    }
}

/// A stock copy mounted read-only as the stock library, the only library.
fn app_with_stock_copy(tmp: &TempDir) -> (Signex, PathBuf) {
    copy_dir(&stock_source(), &tmp.path().join("stock"));
    let stock = tmp.path().join("stock/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    // The world's copy is the stock library, as `library_stock_copy.rs`
    // sets it up; any other copy carrying the stock id is refused (#1894).
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    (app, stock)
}

fn picker(msg: PickerMsg) -> Message {
    Message::Library(LibraryMessage::Picker(msg))
}

fn first_row(app: &Signex, lib: &Path) -> ComponentsPanelRowKey {
    let open = app.library.library_at(lib).expect("mounted");
    let (table, rows) = open.tables.iter().next().expect("a table");
    ComponentsPanelRowKey {
        library_path: lib.to_path_buf(),
        table: table.clone(),
        row_id: signex_library::RowId::from_uuid(rows[0].row_id),
    }
}

fn preview_tabs(app: &Signex) -> usize {
    app.document_state
        .tabs
        .iter()
        .filter(|t| matches!(t.kind, TabKind::ComponentEditor(..)))
        .count()
}

#[test]
fn the_library_row_toggle_expands_and_collapses_it() {
    let tmp = TempDir::new().unwrap();
    let (mut app, stock) = app_with_stock_copy(&tmp);
    assert!(!app.library.panel_expanded.contains(&stock));

    settle(
        &mut app,
        picker(PickerMsg::PanelLibraryToggled(stock.clone())),
    );
    assert!(app.library.panel_expanded.contains(&stock), "expanded");

    settle(
        &mut app,
        picker(PickerMsg::PanelLibraryToggled(stock.clone())),
    );
    assert!(
        !app.library.panel_expanded.contains(&stock),
        "collapsed again"
    );
}

#[test]
fn one_click_on_a_listed_component_highlights_it_and_opens_nothing() {
    let tmp = TempDir::new().unwrap();
    let (mut app, stock) = app_with_stock_copy(&tmp);
    let key = first_row(&app, &stock);

    settle(
        &mut app,
        picker(PickerMsg::PanelComponentClicked(key.clone())),
    );

    assert_eq!(app.library.panel_selected.as_ref(), Some(&key));
    assert_eq!(preview_tabs(&app), 0, "a single click opens no tab");
}

#[test]
fn a_double_click_on_a_listed_component_opens_its_preview() {
    let tmp = TempDir::new().unwrap();
    let (mut app, stock) = app_with_stock_copy(&tmp);
    let key = first_row(&app, &stock);

    settle(
        &mut app,
        picker(PickerMsg::PanelComponentClicked(key.clone())),
    );
    settle(&mut app, picker(PickerMsg::PanelComponentClicked(key)));

    assert_eq!(preview_tabs(&app), 1, "the second click opens the preview");
}

#[test]
fn a_copy_of_a_mounted_library_is_refused_and_the_original_stays() {
    let tmp = TempDir::new().unwrap();
    let (mut app, stock) = app_with_stock_copy(&tmp);
    copy_dir(&stock_source(), &tmp.path().join("copy"));
    let copy = tmp.path().join("copy/Stock.snxlib");

    let refused = app.library.open_library(copy.clone());

    let err = refused.expect_err("a second library with the same id must not mount");
    let message = err.to_string();
    assert!(message.contains("copy of the bundled stock"), "{message}");
    assert!(app.library.library_at(&copy).is_none());
    assert!(app.library.library_at(&stock).is_some(), "the first stays");
    assert_eq!(app.library.open_libraries.len(), 1);
}

/// #1894 review: a stock copy that finishes mounting before the bundled
/// stock library must not win. It is refused and Stock still mounts, so the
/// outcome no longer depends on which mount lands first.
#[test]
fn a_stock_copy_arriving_first_is_refused_and_stock_still_mounts() {
    let tmp = TempDir::new().unwrap();
    copy_dir(&stock_source(), &tmp.path().join("stock"));
    copy_dir(&stock_source(), &tmp.path().join("copy"));
    let stock = tmp.path().join("stock/Stock.snxlib");
    let copy = tmp.path().join("copy/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    // The bundled stock is registered and its mount is in flight, as at
    // startup; the copy's mount lands first.
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .request_mount(&stock, signex_app::library::mount::MountIntent::Silent);

    let err = app
        .library
        .open_library(copy.clone())
        .expect_err("a copy of the stock library is refused");
    assert!(
        err.to_string().contains("copy of the bundled stock"),
        "{err}"
    );

    app.library
        .open_library_read_only(stock.clone())
        .expect("the bundled stock library still mounts");
    assert!(app.library.library_at(&stock).is_some());
    assert!(app.library.library_at(&copy).is_none());
}

/// Review of #1895: closing a library drops what the Components and Library
/// panels held for it, so its details and row state do not outlive it.
#[test]
fn closing_a_library_clears_its_panel_state() {
    let tmp = TempDir::new().unwrap();
    let (mut app, stock) = app_with_stock_copy(&tmp);
    let key = first_row(&app, &stock);
    settle(
        &mut app,
        picker(PickerMsg::PanelLibraryToggled(stock.clone())),
    );
    settle(
        &mut app,
        picker(PickerMsg::PanelComponentClicked(key.clone())),
    );
    app.library.components_panel.selected = Some(key.clone());
    app.library.components_panel.hovered = Some(key.clone());
    app.library.components_panel.details.key = Some(key);

    settle(
        &mut app,
        Message::Library(LibraryMessage::CloseLibrary(stock.clone())),
    );

    assert!(app.library.library_at(&stock).is_none(), "closed");
    let panel = &app.library.components_panel;
    assert!(panel.selected.is_none());
    assert!(panel.hovered.is_none());
    assert!(panel.details.key.is_none());
    assert!(!app.library.panel_expanded.contains(&stock));
    assert!(app.library.panel_selected.is_none());
    assert!(app.library.panel_last_click.is_none());
}

/// Re-review of #1894: when the bundled stock failed to open (a broken
/// install), a copy is what keeps stock references resolving, so it mounts.
#[test]
fn a_stock_copy_mounts_when_the_bundled_stock_failed_to_open() {
    let tmp = TempDir::new().unwrap();
    copy_dir(&stock_source(), &tmp.path().join("copy"));
    let missing = tmp.path().join("gone/Stock.snxlib");
    let copy = tmp.path().join("copy/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    app.library.stock_library_root = Some(missing);
    app.library.pending_mounts.clear();

    app.library
        .open_library(copy.clone())
        .expect("with no bundled stock mounted or mounting, the copy mounts");
    assert!(app.library.library_at(&copy).is_some());
}

/// Review of #1906: a copy refused while the bundled stock was mounting is
/// mounted after all when that mount then fails, so stock references keep
/// resolving whichever mount finished first.
#[test]
fn a_stock_copy_refused_while_stock_mounts_takes_over_when_stock_then_fails() {
    let tmp = TempDir::new().unwrap();
    copy_dir(&stock_source(), &tmp.path().join("copy"));
    let missing = tmp.path().join("gone/Stock.snxlib");
    let copy = tmp.path().join("copy/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    app.library.pending_mounts.clear();
    app.library.stock_library_root = Some(missing.clone());
    app.library
        .request_mount(&missing, signex_app::library::mount::MountIntent::Silent);
    assert!(
        app.library.open_library(copy.clone()).is_err(),
        "refused while the bundled stock mounts"
    );

    for m in run(signex_app::library::mount::spawn_pending_mounts(vec![
        missing,
    ])) {
        settle(&mut app, m);
    }

    assert!(
        app.library.library_at(&copy).is_some(),
        "the bundled stock failed: the refused copy keeps stock references resolving"
    );
}
