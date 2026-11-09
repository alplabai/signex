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
            |d| {
                let (mut app, t) = fixture(d, Kind::Schematic, true);
                app.document_state.tabs.push(t);
                app
            },
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
        // A half-typed buffer is unsaved work: outside the title count like
        // every row, but it enables Save All (#2132).
        (
            "half-typed row only",
            |d| fixture(d, Kind::HalfTypedRow, true).0,
            0,
            true,
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
fn the_enumerator_lists_dirty_and_half_typed_rows_apart_from_files() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fixture(dir.path(), Kind::Row, true);
    let all = app.unsaved_set();
    assert!(all.files.is_empty());
    assert_eq!(all.rows.len(), 1);
    // A half-typed value is unsaved work too (#2132).
    let (app, _) = fixture(dir.path(), Kind::HalfTypedRow, true);
    let half = app.unsaved_set();
    assert!(half.files.is_empty());
    assert_eq!(half.rows.len(), 1);
}

/// One app holding every kind at once: a sheet with a tab, a parked sheet,
/// a symbol and a footprint draft, a dirty row, a half-typed-only row, the
/// project file and a library manifest. Returns the paths the quit prompt
/// must list and the two rows it must list (the dirty one and the
/// half-typed one, #2132).
fn every_kind() -> (
    Signex,
    Vec<PathBuf>,
    Vec<crate::library::state::EditorAddress>,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let d = dir.path();
    let (mut app, sheet_tab) = fixture(d, Kind::Schematic, true);
    let sheet_path = sheet_tab.path.clone();
    app.document_state.tabs.push(sheet_tab);
    let parked = d.join("parked.snxsch");
    app.document_state.insert_engine(
        parked.clone(),
        signex_engine::Engine::new(Default::default()).unwrap(),
    );
    app.document_state.mark_document_dirty(&parked);
    let (symbols, sym_tab) = {
        let (a, t) = fixture(d, Kind::Symbol, true);
        (a.document_state.symbol_editors, t)
    };
    app.document_state.symbol_editors = symbols;
    let (fps, fp_tab) = {
        let (a, t) = fixture(d, Kind::Footprint, true);
        (a.document_state.footprint_editors, t)
    };
    app.document_state.footprint_editors = fps;
    let (row_app, _) = fixture(d, Kind::Row, true);
    let row = row_app.library.editors.keys().next().unwrap().clone();
    app.library.editors = row_app.library.editors;
    let (half, _) = fixture(other.path(), Kind::HalfTypedRow, true);
    let half_row = half.library.editors.keys().next().unwrap().clone();
    app.library.editors.extend(half.library.editors);
    let project = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&project);
    let manifest = d.join("lib.snxlib");
    app.document_state.mark_document_dirty(&manifest);
    let mut want = vec![
        sheet_path,
        parked,
        sym_tab.path,
        fp_tab.path,
        project,
        manifest,
    ];
    want.sort();
    (app, want, vec![row, half_row], dir, other)
}

#[test]
fn the_quit_prompt_for_every_kind_at_once_lists_files_sorted_and_the_unsaved_rows() {
    let (mut app, want, rows, _d, _o) = every_kind();
    let _ = app.update(crate::app::Message::Window(
        crate::app::WindowMsg::CloseMainWindow,
    ));
    let modal = app.ui_state.app_quit_confirm.as_ref().unwrap();
    assert_eq!(modal.dirty_paths, want);
    assert_eq!(modal.dirty_rows.len(), rows.len(), "{:?}", modal.dirty_rows);
    for row in &rows {
        assert!(
            modal.dirty_rows.contains(row),
            "{row:?} in {:?}",
            modal.dirty_rows
        );
    }
}

fn open_quit_prompt(app: &mut Signex) {
    let _ = app.update(crate::app::Message::Window(
        crate::app::WindowMsg::CloseMainWindow,
    ));
    assert!(app.ui_state.app_quit_confirm.is_some());
}

fn set_every_row_dirty(app: &mut Signex, dirty: bool) {
    for editor in app.library.editors.values_mut() {
        editor.dirty = dirty;
    }
}

#[test]
fn a_row_saved_behind_the_quit_prompt_is_pruned_and_the_prompt_closes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fixture(dir.path(), Kind::Row, true);
    open_quit_prompt(&mut app);
    set_every_row_dirty(&mut app, false);
    let _answer = app.prune_settled_quit_confirm();
    assert!(app.ui_state.app_quit_confirm.is_none());
}

#[test]
fn a_row_dirtied_behind_the_quit_prompt_is_not_discarded_and_is_listed_again() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fixture(dir.path(), Kind::Row, false);
    app.document_state
        .mark_document_dirty(&dir.path().join("lib.snxlib"));
    open_quit_prompt(&mut app);
    assert!(
        app.ui_state
            .app_quit_confirm
            .as_ref()
            .unwrap()
            .dirty_rows
            .is_empty()
    );
    set_every_row_dirty(&mut app, true);
    let _ = app.handle_app_quit_confirm(crate::app::ProjectCloseChoice::DiscardAll);
    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("prompt stays up");
    assert_eq!(modal.dirty_rows.len(), 1);
    assert!(app.ui_state.quit_confirm_refreshed);
}
