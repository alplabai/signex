//! #2629: a read made stale while it ran starts again on the project of the
//! sheet it started from, never on whatever the main window shows by then.
//! Number Sheets asked for in an undocked window starts from that window's
//! sheet, so the restart must keep it.

use std::path::{Path, PathBuf};

use super::SheetNumberingMsg;
use super::state::NumberingProblem;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};

/// ProjectA in `dir` with its root sheet active; the dialog is open and its
/// first read is in flight. Returns the root sheet's path.
fn reading(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
    let root = dir.join("root.snxsch");
    app.document_state.active_path = Some(root.clone());
    let _ = app.update(Message::from(SheetNumberingMsg::Requested));
    let state = app
        .ui_state
        .sheet_numbering
        .as_ref()
        .expect("the dialog opened");
    assert_eq!(state.source, root);
    (app, root)
}

/// The in-flight read comes back after a sheet opened or closed.
fn answer_stale(app: &mut Signex) -> u64 {
    let generation = app
        .ui_state
        .sheet_numbering
        .as_ref()
        .expect("open")
        .generation;
    app.document_state.documents_epoch += 1;
    let result = Err(NumberingProblem::ReadFailed("not read".into()));
    let _ = app.update(Message::from(SheetNumberingMsg::Read {
        generation,
        result,
    }));
    generation
}

#[test]
fn a_stale_read_restarts_on_its_sheet_while_the_main_window_has_no_project() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root) = reading(dir.path());
    app.document_state.active_path = None;
    app.document_state.active_project = None;
    let first = answer_stale(&mut app);
    let state = app.ui_state.sheet_numbering.as_ref().expect("still open");
    assert!(state.generation > first, "a new read started");
    assert_eq!(state.source, root);
    assert_eq!(state.project_name, "ProjectA");
}

#[test]
fn a_stale_read_restarts_on_its_own_project_not_the_main_windows() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let (mut app, root) = reading(dir.path());
    let mut project_b = app.document_state.projects[0].clone();
    project_b.id = app.document_state.mint_project_id();
    project_b.path = other.path().join("ProjectB.snxprj");
    project_b.data.name = "ProjectB".into();
    project_b.data.dir = other.path().to_string_lossy().into_owned();
    let b = project_b.id;
    app.document_state.projects.push(project_b);
    app.document_state.active_project = Some(b);
    app.document_state.active_path = Some(other.path().join("root.snxsch"));
    answer_stale(&mut app);
    let state = app.ui_state.sheet_numbering.as_ref().expect("still open");
    assert_eq!(state.source, root);
    assert_eq!(state.project_name, "ProjectA");
}

#[test]
fn a_stale_read_whose_project_closed_closes_the_dialog_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root) = reading(dir.path());
    app.document_state.projects.clear();
    app.document_state.active_project = None;
    // Counted before the answer, so the logger is installed when it lands.
    let needle = format!("{} is no longer in an open project", root.display());
    let before = crate::diagnostics::logged_count(&needle);
    answer_stale(&mut app);
    assert!(app.ui_state.sheet_numbering.is_none());
    assert_eq!(
        crate::diagnostics::logged_count(&needle),
        before + 1,
        "Messages says the dialog closed"
    );
}
