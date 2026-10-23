//! Drive Close Project to its confirm gate without an iced runtime.
//!
//! Close Project reads sibling sheets off-thread: the click returns a `Task`
//! and the gate runs when `ProjectMsg::CloseRefsResolved` comes back. These
//! tests feed that completion by hand, the way the other async regressions
//! (`file_open_async.rs`) feed theirs.

use signex_app::app::{ContextMenuMsg, Message, ProjectMsg, ProjectTreeAction, Signex};

/// Click Close Project on the first project and complete its resolve task:
/// run the task body and send the result back as the completion message.
pub fn close_first_project(app: &mut Signex) {
    let project_id = app.document_state.projects[0].id;
    let task = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![0]),
    )));
    assert_eq!(task.units(), 1, "Close Project must start the resolve task");
    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "the gate must wait for the task, not read the disk in update",
    );
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id,
        result,
    }));
}
