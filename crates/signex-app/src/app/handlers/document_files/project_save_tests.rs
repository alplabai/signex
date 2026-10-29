//! #859 — the `.snxprj` save runs off `update`: nothing is created or written
//! until the returned task runs, and its completion applies the result only
//! to the project that asked.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectCloseChoice, ProjectMsg, Signex};
use crate::library::commands::PendingLibrarySpec;
use crate::persist::run_task::outputs;

struct Fx {
    app: Signex,
    id: ProjectId,
    prj: PathBuf,
    lib: PathBuf,
    lib_id: uuid::Uuid,
}

/// A dirty project with one pending library `New.snxlib` beside it.
fn fixture(dir: &Path) -> Fx {
    let mut app = app_workspace(dir.to_str().expect("utf8"), &[]);
    let id = app.document_state.projects[0].id;
    let prj = app.document_state.projects[0].path.clone();
    let lib = dir.join("New.snxlib");
    let lib_id = uuid::Uuid::now_v7();
    app.document_state.projects[0].pending_libraries.insert(
        lib_id,
        PendingLibrarySpec {
            lib_path: lib.clone(),
            enable_git: false,
            use_lfs: false,
            display_name: "New".into(),
        },
    );
    app.document_state.mark_document_dirty(&prj);
    Fx {
        app,
        id,
        prj,
        lib,
        lib_id,
    }
}

/// Run a save task to completion and return what it reported, undelivered.
fn run(task: iced::Task<Message>) -> Message {
    let mut messages = outputs(task);
    assert_eq!(messages.len(), 1, "one completion per save");
    messages.remove(0)
}

fn deliver(app: &mut Signex, message: Message) -> iced::Task<Message> {
    app.update(message)
}

/// A needle for the Messages ring that is unique to this test's directory
/// and sits early in the line (the ring keeps 157 bytes of each entry).
fn dir_needle(dir: &Path) -> String {
    dir.file_name()
        .expect("dir name")
        .to_string_lossy()
        .into_owned()
}

fn is_dirty(fx: &Fx) -> bool {
    fx.app.document_state.is_document_dirty(&fx.prj)
}

#[test]
fn begin_creates_and_writes_nothing_until_the_task_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());

    let task = fx.app.begin_project_save(fx.id).expect("planned");

    assert!(!fx.lib.exists(), "no git init inside update");
    assert!(!fx.prj.exists(), "no project write inside update");
    assert!(fx.app.document_state.document_saves.is_saving(&fx.prj));
    assert!(
        fx.app.document_state.projects[0]
            .pending_libraries
            .contains_key(&fx.lib_id),
        "the model is untouched until the completion"
    );
    drop(task);
}

#[test]
fn the_completion_registers_the_library_and_cleans_the_project() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let task = fx.app.begin_project_save(fx.id).expect("planned");

    let done = run(task);
    // The worker has run; the model has not heard about it yet.
    assert!(fx.lib.exists() && fx.prj.exists());
    assert!(fx.app.document_state.projects[0].data.libraries.is_empty());
    assert!(is_dirty(&fx));

    drop(deliver(&mut fx.app, done));

    let project = &fx.app.document_state.projects[0];
    assert!(project.pending_libraries.is_empty());
    assert_eq!(project.data.libraries.len(), 1);
    assert_eq!(project.data.libraries[0].library_id, Some(fx.lib_id));
    assert!(!is_dirty(&fx), "clean after the confirmed write");
    assert!(!fx.app.document_state.document_saves.is_saving(&fx.prj));
    let on_disk = signex_types::project::parse_project(&fx.prj).unwrap();
    assert_eq!(on_disk.libraries.len(), 1, "the file lists the library");
}

#[test]
fn an_edit_while_the_write_is_in_flight_keeps_the_project_dirty() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let task = fx.app.begin_project_save(fx.id).expect("planned");
    let done = run(task);

    fx.app.document_state.projects[0].data.description = Some("typed meanwhile".into());
    drop(deliver(&mut fx.app, done));

    assert!(is_dirty(&fx), "the bytes on disk lack the edit");
    assert_eq!(fx.app.document_state.projects[0].data.libraries.len(), 1);
    let on_disk = signex_types::project::parse_project(&fx.prj).unwrap();
    assert_eq!(on_disk.description, None);
}

#[test]
fn a_library_that_cannot_be_created_stays_pending_and_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    std::fs::create_dir_all(&fx.lib).unwrap();
    let needle = dir_needle(tmp.path());
    let before = crate::diagnostics::logged_count(&needle);
    let task = fx.app.begin_project_save(fx.id).expect("planned");

    drop(deliver(&mut fx.app, run(task)));

    let project = &fx.app.document_state.projects[0];
    assert!(project.pending_libraries.contains_key(&fx.lib_id));
    assert!(project.data.libraries.is_empty());
    assert!(is_dirty(&fx), "never clean while a library is pending");
    assert!(fx.app.document_state.error_notice.is_some());
    assert!(
        crate::diagnostics::logged_count(&needle) > before,
        "the failure reaches the Messages panel"
    );
}

#[test]
fn a_completion_for_a_closed_project_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let needle = dir_needle(tmp.path());
    let task = fx.app.begin_project_save(fx.id).expect("planned");
    let done = run(task);

    fx.app.document_state.projects.clear();
    drop(deliver(&mut fx.app, done));

    assert!(fx.app.document_state.projects.is_empty());
    assert!(!fx.app.document_state.document_saves.is_saving(&fx.prj));
    let reported = crate::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("was created, but project") && e.message.contains(&needle));
    assert!(reported, "the orphaned library is reported");
}

#[test]
fn a_completion_for_a_reopened_project_does_not_touch_the_new_one() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let task = fx.app.begin_project_save(fx.id).expect("planned");
    let done = run(task);

    // Closed and reopened at the same path: a different loaded project.
    let mut reopened = fx.app.document_state.projects.remove(0);
    reopened.id = fx.app.document_state.mint_project_id();
    reopened.pending_libraries.clear();
    fx.app.document_state.projects.push(reopened);
    drop(deliver(&mut fx.app, done));

    assert!(fx.app.document_state.projects[0].data.libraries.is_empty());
}

#[test]
fn a_save_while_one_is_running_queues_one_follow_up_and_never_a_second_writer() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let first = fx.app.begin_project_save(fx.id).expect("planned");

    let second = fx.app.begin_project_save(fx.id).expect("queued");
    let third = fx.app.begin_project_save(fx.id).expect("coalesced");
    assert!(outputs(second).is_empty() && outputs(third).is_empty());

    let done = run(first);
    fx.app.document_state.projects[0].data.description = Some("later".into());
    let follow_up = deliver(&mut fx.app, done);

    let messages = outputs(follow_up);
    let writes = messages
        .iter()
        .filter(|m| matches!(m, Message::Project(ProjectMsg::FileSaved(_))))
        .count();
    assert_eq!(writes, 1, "exactly one follow-up write");
    for message in messages {
        drop(deliver(&mut fx.app, message));
    }
    assert!(!is_dirty(&fx));
    let on_disk = signex_types::project::parse_project(&fx.prj).unwrap();
    assert_eq!(on_disk.description.as_deref(), Some("later"));
}

#[test]
fn a_project_save_refused_by_a_path_op_starts_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    fx.app.document_state.pending_path_ops.begin(
        crate::app::path_ops::PathOpKind::Delete,
        vec![fx.prj.clone()],
        crate::app::state::CaseFold::PRESERVE,
    );

    assert!(fx.app.begin_project_save(fx.id).is_err());

    assert!(!fx.app.document_state.document_saves.is_saving(&fx.prj));
    assert!(!fx.lib.exists());
}

#[test]
fn a_successful_write_moves_the_documents_epoch() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let before = fx.app.document_state.documents_epoch;
    let task = fx.app.begin_project_save(fx.id).expect("planned");

    drop(deliver(&mut fx.app, run(task)));

    assert!(fx.app.document_state.documents_epoch > before);
}

/// Quit with Save All settles only after the project write completed: the
/// project is clean and the library exists when the quit is decided.
#[test]
fn quit_save_all_waits_for_the_project_write() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let _ = fx.app.handle_app_quit_requested();
    let task = fx.app.handle_app_quit_confirm(ProjectCloseChoice::SaveAll);

    let messages = outputs(task);
    let saved = messages
        .iter()
        .position(|m| matches!(m, Message::Project(ProjectMsg::FileSaved(_))))
        .expect("the project write completes inside the Save All task");
    let settled = messages
        .iter()
        .position(|m| matches!(m, Message::File(crate::app::FileMsg::SaveAllSettled(_))))
        .expect("settle");
    assert!(saved < settled, "settle comes after the write");
    for message in messages {
        drop(deliver(&mut fx.app, message));
    }

    assert!(fx.app.document_state.error_notice.is_none());
    assert!(!is_dirty(&fx));
    assert!(fx.lib.exists());
}

/// A Save All issued while a project write is still running does not decide
/// the quit on that write: it keeps Signex open and says why.
#[test]
fn quit_save_all_with_a_write_in_flight_stays_open() {
    let tmp = tempfile::tempdir().unwrap();
    let mut fx = fixture(tmp.path());
    let running = fx.app.begin_project_save(fx.id).expect("planned");
    let _ = fx.app.handle_app_quit_requested();
    let task = fx.app.handle_app_quit_confirm(ProjectCloseChoice::SaveAll);
    for message in outputs(task) {
        drop(deliver(&mut fx.app, message));
    }
    assert!(fx.app.document_state.error_notice.is_some(), "stays open");
    assert!(is_dirty(&fx));

    drop(deliver(&mut fx.app, run(running)));
    assert!(!is_dirty(&fx), "the running write still lands");
}
