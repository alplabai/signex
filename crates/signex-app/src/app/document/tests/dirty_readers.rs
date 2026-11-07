//! Guards for the three readers of "what is dirty": the title-bar count,
//! Save All's enablement and the quit prompt's lists. They pin today's
//! answers per kind, so moving the readers onto one enumerator (#661) is
//! provably behaviour-preserving.

use std::path::{Path, PathBuf};

use super::{Kind, fixture, save_in_flight};
use crate::app::Signex;

/// Title with `n` unsaved documents, as the title bar words it.
fn title_for(n: usize) -> String {
    let version = env!("CARGO_PKG_VERSION");
    if n == 0 {
        format!("Signex {version}")
    } else {
        format!("• Signex {version} — {n} unsaved")
    }
}

fn title(app: &Signex) -> String {
    app.title(iced::window::Id::unique())
}

fn save_all_enabled(app: &Signex) -> bool {
    app.command_context(None).has_dirty_documents
}

fn manifest_only(dir: &Path) -> Signex {
    let (mut app, _) = fixture(dir, Kind::Project, false);
    app.document_state
        .mark_document_dirty(&dir.join("lib.snxlib"));
    app
}

type Build = fn(&Path) -> Signex;

fn two_kinds(dir: &Path) -> Signex {
    let (mut app, _) = fixture(dir, Kind::Schematic, true);
    let (other, _) = fixture(dir, Kind::Symbol, true);
    app.document_state.symbol_editors = other.document_state.symbol_editors;
    app
}

/// (case, build, unsaved count in the title, Save All enabled)
fn cases() -> Vec<(&'static str, Build, usize, bool)> {
    vec![
        ("clean", |d| fixture(d, Kind::Schematic, false).0, 0, false),
        (
            "dirty sheet tab",
            |d| fixture(d, Kind::Schematic, true).0,
            1,
            true,
        ),
        (
            "parked dirty sheet",
            |d| fixture(d, Kind::Schematic, true).0,
            1,
            true,
        ),
        (
            "dirty symbol draft",
            |d| fixture(d, Kind::Symbol, true).0,
            1,
            true,
        ),
        (
            "dirty footprint draft",
            |d| fixture(d, Kind::Footprint, true).0,
            1,
            true,
        ),
        (
            "symbol save in flight",
            |d| {
                let (mut app, t) = fixture(d, Kind::Symbol, false);
                app.document_state.keep_document_dirty_until_saved(&t.path);
                app
            },
            1,
            true,
        ),
        (
            "footprint save in flight",
            |d| {
                let (mut app, t) = fixture(d, Kind::Footprint, false);
                app.document_state.keep_document_dirty_until_saved(&t.path);
                app
            },
            1,
            true,
        ),
        // Rows are excluded from the title count but enable Save All.
        ("dirty row only", |d| fixture(d, Kind::Row, true).0, 0, true),
        // A half-typed buffer is unsaved input, not a dirty row.
        (
            "half-typed row only",
            |d| fixture(d, Kind::HalfTypedRow, true).0,
            0,
            false,
        ),
        (
            "dirty project file",
            |d| fixture(d, Kind::Project, true).0,
            1,
            true,
        ),
        ("dirty library manifest only", manifest_only, 1, true),
        ("sheet and symbol draft", two_kinds, 2, true),
    ]
}

#[test]
fn the_title_count_and_save_all_enablement_per_kind() {
    for (name, build, count, enabled) in cases() {
        let dir = tempfile::tempdir().unwrap();
        let app = build(dir.path());
        assert_eq!(title(&app), title_for(count), "{name}: title");
        assert_eq!(save_all_enabled(&app), enabled, "{name}: Save All");
    }
}

/// The prompt the quit request opens, as (paths, rows).
fn quit_lists(app: &mut Signex) -> (Vec<PathBuf>, usize) {
    let _ = app.update(crate::app::Message::Window(
        crate::app::WindowMsg::CloseMainWindow,
    ));
    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("a dirty document opens the quit prompt");
    (modal.dirty_paths.clone(), modal.dirty_rows.len())
}

#[test]
fn quit_lists_a_parked_sheet_that_has_no_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, t) = fixture(dir.path(), Kind::Schematic, true);
    assert!(app.document_state.tabs.is_empty());
    assert_eq!(quit_lists(&mut app), (vec![t.path], 0));
}

#[test]
fn quit_lists_a_draft_whose_save_is_in_flight() {
    for kind in [Kind::Symbol, Kind::Footprint] {
        let (mut app, t, _dir) = save_in_flight(kind);
        assert_eq!(quit_lists(&mut app), (vec![t.path], 0), "{kind:?}");
    }
}

#[test]
fn quit_lists_a_dirty_library_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = manifest_only(dir.path());
    let want = dir.path().join("lib.snxlib");
    let (paths, rows) = quit_lists(&mut app);
    assert!(paths.contains(&want), "{paths:?}");
    assert_eq!(rows, 0);
}

#[test]
fn the_enumerator_keys_each_file_by_its_owner_and_leaves_ownerless_members_bare() {
    use crate::app::document::DocumentKey as K;
    let key_of = |app: &Signex, path: &Path| {
        let files = app.document_state.unsaved_files();
        let hit: Vec<_> = files.iter().filter(|f| f.path == path).collect();
        assert_eq!(hit.len(), 1, "{path:?} once in {files:?}");
        hit[0].key.clone()
    };
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();

    let (app, t) = fixture(d, Kind::Schematic, true);
    assert_eq!(key_of(&app, &t.path), Some(K::Schematic(t.path.clone())));
    let (app, t) = fixture(d, Kind::Pcb, true);
    assert_eq!(key_of(&app, &t.path), Some(K::Pcb(t.path.clone())));
    let (app, t) = fixture(d, Kind::Symbol, true);
    assert_eq!(key_of(&app, &t.path), Some(K::Symbol(t.path.clone())));
    let (app, t) = fixture(d, Kind::Footprint, true);
    assert_eq!(key_of(&app, &t.path), Some(K::Footprint(t.path.clone())));
    let (app, t) = fixture(d, Kind::Project, true);
    let id = app.document_state.projects[0].id;
    assert_eq!(key_of(&app, &t.path), Some(K::Project(id)));

    // A save in flight is listed once, keyed by its draft.
    let (app, t, _dir) = save_in_flight(Kind::Symbol);
    assert_eq!(key_of(&app, &t.path), Some(K::Symbol(t.path.clone())));

    // A manifest has no document: path only.
    let manifest = d.join("lib.snxlib");
    let app = manifest_only(d);
    assert_eq!(key_of(&app, &manifest), None);
}

#[test]
fn the_enumerator_lists_dirty_rows_apart_and_not_half_typed_ones() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fixture(dir.path(), Kind::Row, true);
    let all = app.unsaved_set();
    assert!(all.files.is_empty());
    assert_eq!(all.rows.len(), 1);
    let (app, _) = fixture(dir.path(), Kind::HalfTypedRow, true);
    assert!(app.unsaved_set().is_empty());
}
