//! #1205 — New Project creates its own `<stem>/` folder and never writes
//! into a folder that is not empty or already holds a project.

use std::fs;
use std::path::Path;

use signex_app::app::{FileMsg, Message, Signex};
use tempfile::TempDir;

fn new_project(app: &mut Signex, chosen: &Path) {
    crate::settle_support::settle(
        app,
        Message::File(FileMsg::NewProject(Some(chosen.to_path_buf()))),
    );
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn new_project_creates_a_folder_named_after_the_project() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &tmp.path().join("Fresh.snxprj"));

    let folder = tmp.path().join("Fresh");
    assert!(folder.join("Fresh.snxprj").is_file());
    assert!(folder.join("Fresh.snxsch").is_file());
    assert!(folder.join(".gitignore").is_file());
    assert_eq!(
        names_in(tmp.path()),
        ["Fresh"],
        "nothing lands beside the folder in the chosen location"
    );
    assert!(app.document_state.error_notice.is_none());
    let project = app.document_state.projects.last().expect("project opened");
    assert_eq!(project.path, folder.join("Fresh.snxprj"));
}

#[test]
fn an_empty_folder_already_named_after_the_project_is_used_directly() {
    let tmp = TempDir::new().unwrap();
    let folder = tmp.path().join("Foo");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join(".DS_Store"), b"clutter").unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &folder.join("Foo.snxprj"));

    assert!(folder.join("Foo.snxprj").is_file());
    assert!(folder.join("Foo.snxsch").is_file());
    assert!(!folder.join("Foo").exists(), "no Foo/Foo/ nesting");
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_non_empty_target_folder_is_refused_and_nothing_is_written() {
    let tmp = TempDir::new().unwrap();
    let folder = tmp.path().join("Busy");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("notes.txt"), b"mine").unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &tmp.path().join("Busy.snxprj"));

    assert_eq!(names_in(&folder), ["notes.txt"]);
    assert_eq!(fs::read(folder.join("notes.txt")).unwrap(), b"mine");
    let notice = app.document_state.error_notice.as_ref().expect("refused");
    assert!(notice.detail.contains("not empty"), "{notice:?}");
    assert!(app.document_state.projects.is_empty());
}

#[test]
fn a_target_folder_that_holds_a_project_is_refused_untouched() {
    let tmp = TempDir::new().unwrap();
    let folder = tmp.path().join("Keep");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("Other.snxprj"), b"precious").unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &tmp.path().join("Keep.snxprj"));

    assert_eq!(names_in(&folder), ["Other.snxprj"]);
    assert_eq!(fs::read(folder.join("Other.snxprj")).unwrap(), b"precious");
    let notice = app.document_state.error_notice.as_ref().expect("refused");
    assert!(notice.detail.contains("already"), "{notice:?}");
}

#[test]
fn choosing_the_existing_project_inside_its_own_folder_is_refused() {
    let tmp = TempDir::new().unwrap();
    let folder = tmp.path().join("Keep");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("Keep.snxprj"), b"precious").unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &folder.join("Keep.snxprj"));

    assert_eq!(names_in(&folder), ["Keep.snxprj"]);
    assert!(!folder.join("Keep").exists(), "no nested project folder");
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn an_existing_project_in_a_shared_folder_still_opens() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Shared.snxprj"), b"").unwrap();
    fs::write(tmp.path().join("holiday.jpg"), b"photo").unwrap();
    let (mut app, _t) = Signex::new();

    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(tmp.path().join("Shared.snxprj")))),
    );

    assert!(app.document_state.error_notice.is_none());
    assert_eq!(app.document_state.projects.len(), 1);
}

#[test]
fn a_project_inside_an_existing_project_is_refused() {
    let tmp = TempDir::new().unwrap();
    let alpha = tmp.path().join("Alpha");
    fs::create_dir(&alpha).unwrap();
    fs::write(alpha.join("Alpha.snxprj"), b"").unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &alpha.join("Beta.snxprj"));

    assert!(!alpha.join("Beta").exists(), "nothing was created");
    let notice = app.document_state.error_notice.as_ref().expect("refused");
    assert!(notice.detail.contains("inside the project"), "{notice:?}");
}

#[test]
fn a_same_named_empty_folder_is_reused_whatever_the_letter_case() {
    let tmp = TempDir::new().unwrap();
    let folder = tmp.path().join("FOO");
    fs::create_dir(&folder).unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &folder.join("foo.snxprj"));

    assert!(folder.join("foo.snxprj").is_file());
    assert!(!folder.join("foo").exists(), "no FOO/foo/ nesting");
}

#[test]
fn a_busy_same_named_folder_gets_a_nested_project_and_says_where() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let folder = tmp.path().join("Foo");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("notes.txt"), b"mine").unwrap();
    let before = signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.starts_with("[new project]"))
        .count();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, &folder.join("Foo.snxprj"));

    assert!(folder.join("Foo").join("Foo.snxprj").is_file());
    assert_eq!(fs::read(folder.join("notes.txt")).unwrap(), b"mine");
    let after = signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.starts_with("[new project]"))
        .count();
    assert_eq!(after - before, 1, "one line says where it was created");
}
