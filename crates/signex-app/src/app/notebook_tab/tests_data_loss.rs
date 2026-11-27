//! The notebook is never lost or written over unseen (#3019): Close Project
//! asks for an unsaved notebook, a clean notebook is read again after an
//! outside change, and Save does not replace a file changed on disk since
//! it was read.

use super::NotebookMsg;
use super::tests::{NOTEBOOK, app_with_notebook, body, is_dirty, notebook_file, send};
use crate::app::{ContextMenuMsg, Message, ProjectMsg, ProjectTreeAction};

fn typed(c: char, id: crate::app::state::ProjectId) -> NotebookMsg {
    NotebookMsg::TextActed {
        project_id: id,
        action: super::edit::typed(c),
    }
}

/// Close Project with the notebook typed in asks first, the notebook among
/// the unsaved files, and keeps the project and its notebook open.
#[test]
fn close_project_asks_for_an_unsaved_notebook() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(&mut app, typed('x', id));
    assert!(is_dirty(&app, id));
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![id.to_string()]),
    )));
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id: id,
        result,
    }));
    let confirm = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("the close asks");
    assert!(
        confirm.dirty_paths.contains(&notebook_file(dir.path())),
        "{:?}",
        confirm.dirty_paths
    );
    assert!(
        app.document_state.notebooks.contains_key(&id),
        "not dropped"
    );
    assert!(app.document_state.projects.iter().any(|p| p.id == id));
}

/// A notebook held with no unsaved edit is read again after the file
/// changed outside (a branch switch, a pull): the tab shows the new text.
#[test]
fn a_clean_notebook_is_read_again_after_an_outside_change() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let path = notebook_file(dir.path());
    let theirs = NOTEBOOK.replace("Power stage", "Charger");
    std::fs::write(&path, &theirs).unwrap();
    let reread = app.reread_clean_notebook(&path).expect("held and clean");
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, reread)
        .expect("settles");
    assert!(body(&app, id).contains("Charger"), "{}", body(&app, id));
    send(&mut app, typed('x', id));
    assert!(
        app.reread_clean_notebook(&path).is_none(),
        "an unsaved notebook is never read over"
    );
}

/// Save does not replace a notebook file something else wrote after the
/// tab read it; the edit stays unsaved and the reason is kept.
#[test]
fn save_does_not_replace_a_file_changed_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let path = notebook_file(dir.path());
    std::fs::write(&path, "theirs").unwrap();
    send(&mut app, typed('x', id));
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theirs");
    assert!(is_dirty(&app, id), "still unsaved");
    let why = app.document_state.notebooks[&id]
        .save_error
        .clone()
        .unwrap_or_default();
    assert!(why.contains("changed on disk"), "{why}");
}
