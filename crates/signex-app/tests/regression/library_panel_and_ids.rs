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
    assert!(app.library.panel_last_click.key().is_none());
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

// ── #1908: one answer for a shared library id, in either arrival order ──

#[derive(Clone, Copy, PartialEq)]
enum Source {
    Project,
    Installed,
    Global,
}

/// Two copies of one library, `strong` ranked above `weak` by how the app
/// knows them. Returns the app, both paths and the project path list.
fn two_sources(
    tmp: &TempDir,
    strong: Source,
    weak: Source,
) -> (Signex, PathBuf, PathBuf, Vec<PathBuf>) {
    copy_dir(&stock_source(), &tmp.path().join("strong"));
    copy_dir(&stock_source(), &tmp.path().join("weak"));
    let strong_path = tmp.path().join("strong/Stock.snxlib");
    let weak_path = tmp.path().join("weak/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    // No bundled stock in play: these are two ordinary libraries.
    app.library.stock_library_root = None;
    app.library.pending_mounts.clear();
    let mut project_paths = Vec::new();
    for (source, path) in [(strong, &strong_path), (weak, &weak_path)] {
        match source {
            Source::Project => project_paths.push(path.clone()),
            Source::Installed => app.library.installed_libraries.push(path.clone()),
            Source::Global => app.library.global_libraries.push(
                signex_app::panels::components_panel::global_prefs::GlobalLibraryEntry {
                    path: path.clone(),
                    remote: None,
                    auto_pull: None,
                },
            ),
        }
    }
    (app, strong_path, weak_path, project_paths)
}

fn land(app: &mut Signex, path: &Path, project_paths: &[PathBuf]) -> Result<(), String> {
    let prepared = signex_app::library::mount::prepare_mount(path).expect("prepare");
    app.library
        .mount_prepared_among(prepared, project_paths)
        .map_err(|e| e.to_string())
}

fn assert_strong_wins(strong: Source, weak: Source, strong_first: bool) {
    let tmp = TempDir::new().unwrap();
    let (mut app, strong_path, weak_path, project_paths) = two_sources(&tmp, strong, weak);
    let order = if strong_first {
        [&strong_path, &weak_path]
    } else {
        [&weak_path, &strong_path]
    };
    let mut results = Vec::new();
    for path in order {
        results.push(land(&mut app, path, &project_paths));
    }
    assert!(
        app.library.library_at(&strong_path).is_some(),
        "strong stays"
    );
    assert!(app.library.library_at(&weak_path).is_none(), "weak is out");
    let (refused, mounted_both) = (results[1].is_err(), results[1].is_ok());
    assert_eq!(refused, strong_first);
    assert_eq!(mounted_both, !strong_first);
    let notice = &app.library.id_clash_notices;
    assert_eq!(notice.len(), 1, "one visible notice: {notice:?}");
    assert_eq!(notice[0].kept, strong_path);
    assert_eq!(notice[0].set_aside, weak_path);
    assert_eq!(notice[0].replaced, !strong_first);
    let text = notice[0].message();
    assert!(text.contains(&strong_path.display().to_string()), "{text}");
    assert!(text.contains(&weak_path.display().to_string()), "{text}");
}

#[test]
fn a_project_library_beats_an_installed_copy_in_either_order() {
    assert_strong_wins(Source::Project, Source::Installed, true);
    assert_strong_wins(Source::Project, Source::Installed, false);
}

#[test]
fn an_installed_library_beats_a_global_copy_in_either_order() {
    assert_strong_wins(Source::Installed, Source::Global, true);
    assert_strong_wins(Source::Installed, Source::Global, false);
}

#[test]
fn a_project_library_beats_a_global_copy_in_either_order() {
    assert_strong_wins(Source::Project, Source::Global, true);
    assert_strong_wins(Source::Project, Source::Global, false);
}

/// A lower-ranked library with unsaved editors is not closed under the
/// user: the newcomer is refused instead.
#[test]
fn a_replacement_never_drops_a_library_with_unsaved_edits() {
    let tmp = TempDir::new().unwrap();
    let (mut app, strong, weak, project_paths) =
        two_sources(&tmp, Source::Project, Source::Installed);
    land(&mut app, &weak, &project_paths).expect("weak mounts first");
    let key = first_row(&app, &weak);
    let address =
        signex_app::library::state::EditorAddress::new(weak.clone(), key.table.clone(), key.row_id);
    let row = app.library.library_at(&weak).unwrap().tables[&key.table][0].clone();
    let mut editor = signex_app::library::state::ComponentPreviewState::from_row(
        weak.clone(),
        key.table.clone(),
        row,
    );
    editor.dirty = true;
    app.library.editors.insert(address, editor);

    assert!(land(&mut app, &strong, &project_paths).is_err());
    assert!(app.library.library_at(&weak).is_some());
    assert!(app.library.library_at(&strong).is_none());
}

/// #1908: renaming a library on disk moves the panels' row keys with it.
#[test]
fn relocating_a_library_rekeys_the_panel_state() {
    let tmp = TempDir::new().unwrap();
    // An ordinary writable library: a locked stock mount cannot be moved.
    copy_dir(&stock_source(), &tmp.path().join("lib"));
    let stock = tmp.path().join("lib/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    app.library.stock_library_root = None;
    app.library.pending_mounts.clear();
    app.library.open_library(stock.clone()).expect("mount");
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
    app.library.components_panel.details.key = Some(key.clone());
    assert!(app.library.panel_expanded.contains(&stock));

    let renamed = stock.with_file_name("Renamed.snxlib");
    std::fs::rename(&stock, &renamed).expect("rename on disk");
    app.library
        .relocate_library(&stock, &renamed)
        .expect("relocate");

    let panel = &app.library.components_panel;
    for moved in [&panel.selected, &panel.hovered, &panel.details.key] {
        assert_eq!(moved.as_ref().unwrap().library_path, renamed);
    }
    assert!(app.library.panel_expanded.contains(&renamed));
    assert!(!app.library.panel_expanded.contains(&stock));
    assert_eq!(
        app.library.panel_selected.as_ref().unwrap().library_path,
        renamed
    );
    assert_eq!(
        app.library.panel_last_click.key().unwrap().library_path,
        renamed
    );
}
