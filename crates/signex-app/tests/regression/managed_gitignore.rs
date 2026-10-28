//! #1117 — New Project writes the managed `.gitignore` block.
//!
//! The merge, link and read-only refusals of an existing `.gitignore` are
//! covered where `ensure_project_gitignore` lives (`signex-types`): since
//! #1205 New Project only ever writes into a fresh, empty folder, so a
//! pre-existing `.gitignore` can no longer reach it.

use signex_app::app::{FileMsg, Message, Signex};
use signex_types::managed_block::{BEGIN_MARKER, END_MARKER};
use signex_types::project_vcs_files::GITIGNORE_BLOCK_BODY;
use tempfile::TempDir;

fn block() -> String {
    format!("{BEGIN_MARKER}\n{GITIGNORE_BLOCK_BODY}{END_MARKER}\n")
}

#[test]
fn new_project_creates_a_gitignore_that_is_exactly_the_managed_block() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();

    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::NewProject(Some(tmp.path().join("Fresh.snxprj")))),
    );

    assert!(app.document_state.error_notice.is_none());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("Fresh").join(".gitignore")).unwrap(),
        block()
    );
}

#[test]
fn a_folder_that_already_has_a_gitignore_is_refused_and_left_as_it_was() {
    let tmp = TempDir::new().unwrap();
    let folder = tmp.path().join("Fresh");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join(".gitignore"), "mine\n").unwrap();
    let (mut app, _t) = Signex::new();

    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::NewProject(Some(tmp.path().join("Fresh.snxprj")))),
    );

    assert!(app.document_state.error_notice.is_some());
    assert_eq!(
        std::fs::read_to_string(folder.join(".gitignore")).unwrap(),
        "mine\n"
    );
    assert!(!folder.join("Fresh.snxprj").exists());
}
