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

/// A stock copy mounted read-only, as the only library.
fn app_with_stock_copy(tmp: &TempDir) -> (Signex, PathBuf) {
    copy_dir(&stock_source(), &tmp.path().join("stock"));
    let stock = tmp.path().join("stock/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
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
    assert!(
        message.contains("same library id") && message.contains("copy"),
        "{message}"
    );
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
    app.library.stock_library_root = Some(stock.clone());

    let err = app
        .library
        .open_library(copy.clone())
        .expect_err("a copy of the stock library is refused");
    assert!(err.to_string().contains("SIGNEX_EDIT_STOCK"), "{err}");

    app.library
        .open_library_read_only(stock.clone())
        .expect("the bundled stock library still mounts");
    assert!(app.library.library_at(&stock).is_some());
    assert!(app.library.library_at(&copy).is_none());
}
