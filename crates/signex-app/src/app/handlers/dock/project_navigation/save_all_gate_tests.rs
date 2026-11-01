//! #1155 — Quit and Close Project must not proceed after a Save All write
//! failed or was refused. Both routes are driven through their real
//! dispatchers (`handle_app_quit_confirm`, `handle_project_close_confirm`);
//! unit tests because `CaseFold` is crate-private, so an integration test
//! cannot build a folding project.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::scope::CaseFold;
use crate::app::{ProjectCloseChoice, Signex};
use crate::library::commands::PendingLibrarySpec;

fn engine_at(path: &Path, paper: &str) -> signex_engine::Engine {
    let sheet = signex_types::schematic::SchematicSheet {
        paper_size: paper.to_string(),
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    engine
}

fn hold_dirty(app: &mut Signex, path: &Path, paper: &str) {
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine_at(path, paper));
    app.document_state.mark_document_dirty(path);
}

/// A project over `dir` listing `X.snxsch`.
fn project_app(dir: &Path) -> Signex {
    app_workspace(dir.to_str().expect("utf8 tempdir"), &["X.snxsch"])
}

/// Quit, press Save All. Returns whether the quit went ahead.
fn quit_save_all(app: &mut Signex) -> bool {
    let _ = app.handle_app_quit_requested();
    assert!(app.ui_state.app_quit_confirm.is_some(), "quit must prompt");
    let task = app.handle_app_quit_confirm(ProjectCloseChoice::SaveAll);
    settle(app, task);
    app.document_state.error_notice.is_none()
}

/// Run a Save All's writes to completion and feed the completions, ending
/// with the settle message that decides whether the quit / close proceeds.
fn settle(app: &mut Signex, task: iced::Task<crate::app::Message>) {
    for message in crate::persist::run_task::outputs(task) {
        let _ = app.update(message);
    }
}

/// Close the first project, press Save All. Returns whether it closed.
fn close_save_all(app: &mut Signex) -> bool {
    let id = app.document_state.projects[0].id;
    let _ = app.close_project_at_tree_path(&[0]);
    let refs = app.resolve_sheet_refs_blocking();
    let _ = app.handle_close_refs_resolved(id, refs);
    assert!(
        app.ui_state.project_close_confirm.is_some(),
        "Close Project must prompt"
    );
    let task = app.handle_project_close_confirm(ProjectCloseChoice::SaveAll);
    settle(app, task);
    app.document_state.projects.iter().all(|p| p.id != id)
}

fn assert_refused(app: &Signex, still_dirty: &[&Path]) {
    assert!(
        app.document_state.error_notice.is_some(),
        "the failure must be reported"
    );
    for path in still_dirty {
        assert!(
            app.document_state.is_document_dirty(path),
            "{} must stay dirty so a retry or Discard All can act on it",
            path.display()
        );
    }
}

/// A pending library whose target already exists cannot be created, so the
/// `.snxprj` would be written without it.
fn add_unmaterialisable_library(app: &mut Signex, dir: &Path) {
    let lib = dir.join("Taken.snxlib");
    std::fs::create_dir_all(&lib).unwrap();
    let spec = PendingLibrarySpec {
        lib_path: lib,
        enable_git: false,
        use_lfs: false,
        display_name: "Taken".into(),
    };
    app.document_state.projects[0]
        .pending_libraries
        .insert(uuid::Uuid::now_v7(), spec);
    let prj = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&prj);
}

#[test]
fn quit_stays_open_when_a_pending_library_could_not_be_created() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = project_app(tmp.path());
    add_unmaterialisable_library(&mut app, tmp.path());
    let prj = app.document_state.projects[0].path.clone();

    assert!(!quit_save_all(&mut app), "quit must not proceed");

    assert_refused(&app, &[&prj]);
}

#[test]
fn close_project_stays_open_when_a_pending_library_could_not_be_created() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = project_app(tmp.path());
    add_unmaterialisable_library(&mut app, tmp.path());
    let prj = app.document_state.projects[0].path.clone();

    assert!(!close_save_all(&mut app), "the project must stay open");

    assert_refused(&app, &[&prj]);
}

/// `X.snxsch` is a directory, so the schematic write cannot succeed.
fn unwritable_sheet(dir: &Path) -> (Signex, PathBuf) {
    let sheet = dir.join("X.snxsch");
    std::fs::create_dir_all(&sheet).unwrap();
    let mut app = project_app(dir);
    hold_dirty(&mut app, &sheet, "A4");
    (app, sheet)
}

#[test]
fn quit_stays_open_when_a_write_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, sheet) = unwritable_sheet(tmp.path());

    assert!(!quit_save_all(&mut app));

    assert_refused(&app, &[&sheet]);
    assert!(app.document_state.engines.contains_key(&sheet));
}

#[test]
fn close_project_stays_open_when_a_write_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, sheet) = unwritable_sheet(tmp.path());

    assert!(!close_save_all(&mut app));

    assert_refused(&app, &[&sheet]);
    assert!(app.document_state.engines.contains_key(&sheet));
}

/// One file held under two spellings with different edits, on a volume that
/// folds case: writing both would silently drop one set of edits.
fn variant_spellings(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let upper = dir.join("X.snxsch");
    let lower = dir.join("x.snxsch");
    std::fs::write(&upper, b"on-disk").unwrap();
    let mut app = project_app(dir);
    app.document_state.projects[0].case_fold = CaseFold::from_probe(true);
    app.document_state.projects[0].case_decided = true;
    hold_dirty(&mut app, &upper, "A4");
    hold_dirty(&mut app, &lower, "A3");
    (app, upper, lower)
}

#[test]
fn quit_stays_open_when_case_variants_disagree() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, upper, lower) = variant_spellings(tmp.path());

    assert!(!quit_save_all(&mut app));

    assert_refused(&app, &[&upper, &lower]);
    assert_eq!(std::fs::read(&upper).unwrap(), b"on-disk");
}

#[test]
fn close_project_stays_open_when_case_variants_disagree() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, upper, lower) = variant_spellings(tmp.path());

    assert!(!close_save_all(&mut app));

    assert_refused(&app, &[&upper, &lower]);
    assert_eq!(std::fs::read(&upper).unwrap(), b"on-disk");
}

#[test]
fn quit_proceeds_when_every_document_saved() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = project_app(tmp.path());
    let sheet = tmp.path().join("X.snxsch");
    hold_dirty(&mut app, &sheet, "A4");

    assert!(quit_save_all(&mut app), "a clean Save All must still quit");

    assert!(!app.document_state.has_dirty_documents());
}

/// #1209 — closing a project drops its stored-rules undo history, as it
/// does its net-class history; the ids are never reused, so the entry would
/// otherwise sit in memory for the rest of the session.
#[test]
fn closing_a_project_drops_its_rules_history() {
    let dir = std::env::temp_dir().join(format!("signex-close-rules-{}", uuid::Uuid::now_v7()));
    let mut app = project_app(&dir);
    let id = app.document_state.projects[0].id;
    app.document_state.rules_history.entry(id).or_default();

    let _ = app.close_project_at_tree_path(&[0]);
    let refs = app.resolve_sheet_refs_blocking();
    let _ = app.handle_close_refs_resolved(id, refs);

    assert!(app.document_state.projects.iter().all(|p| p.id != id));
    assert!(!app.document_state.rules_history.contains_key(&id));
}

/// #1825 — a sheet the user made read-only is never replaced by a save.
mod read_only_sheet {
    use super::*;
    use crate::app::contracts::SaveAllNext;

    fn lock(path: &Path) {
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(path, perms).unwrap();
    }

    /// A project with a read-only `X.snxsch` (holding `locked`) and a
    /// writable `Y.snxsch`, both edited in memory.
    fn app_with_locked_and_free(dir: &Path) -> (Signex, PathBuf, PathBuf) {
        let locked = dir.join("X.snxsch");
        let free = dir.join("Y.snxsch");
        std::fs::write(&locked, b"locked").unwrap();
        std::fs::write(&free, b"free").unwrap();
        lock(&locked);
        let mut app = project_app(dir);
        hold_dirty(&mut app, &locked, "A4");
        hold_dirty(&mut app, &free, "A3");
        (app, locked, free)
    }

    #[test]
    fn save_all_refuses_the_read_only_sheet_and_saves_the_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, locked, free) = app_with_locked_and_free(tmp.path());
        let mut paths = vec![locked.clone(), free.clone()];
        paths.sort();

        let task = app.begin_save_all("Save All", &paths, &[], SaveAllNext::Nothing);
        settle(&mut app, task);

        assert_eq!(std::fs::read(&locked).unwrap(), b"locked");
        assert_ne!(std::fs::read(&free).unwrap(), b"free");
        assert_refused(&app, &[&locked]);
        assert!(!app.document_state.is_document_dirty(&free));
        let notice = format!("{:?}", app.document_state.error_notice);
        assert!(notice.contains("X.snxsch"), "names the file: {notice}");
        assert!(notice.contains("read-only"), "says why: {notice}");
    }

    #[test]
    fn quit_save_all_stays_open_when_a_sheet_is_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, locked, _free) = app_with_locked_and_free(tmp.path());

        assert!(!quit_save_all(&mut app), "quit must not proceed as saved");

        assert_eq!(std::fs::read(&locked).unwrap(), b"locked");
        assert_refused(&app, &[&locked]);
    }

    #[test]
    fn plain_save_keeps_the_read_only_sheet_dirty_and_logs_the_refusal() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, locked, _free) = app_with_locked_and_free(tmp.path());
        let before = crate::diagnostics::logged_count("X.snxsch is read-only");

        let task = app.begin_schematic_save(locked.clone(), None).unwrap();
        settle(&mut app, task);

        assert_eq!(std::fs::read(&locked).unwrap(), b"locked");
        assert_refused(&app, &[&locked]);
        assert!(crate::diagnostics::logged_count("X.snxsch is read-only") > before);
    }
}
