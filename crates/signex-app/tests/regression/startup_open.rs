//! #317 — a document path on the command line opens at startup.
//!
//! The installers register `signex <path>` as the file-association
//! handler. `Signex::boot` turns those paths into `FileMsg::Opened`
//! messages (the File > Open flow); these tests drive that flow the same
//! way the iced runtime would: take the messages `boot` would dispatch and
//! feed them to `update`.

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::startup_args::document_paths;

use std::ffi::OsString;
use std::fs;
use tempfile::TempDir;

fn dispatch_startup(app: &mut Signex, args: &[&std::path::Path], cwd: &std::path::Path) {
    let paths = document_paths(args.iter().map(|a| OsString::from(a.as_os_str())), cwd);
    for message in Signex::startup_open_messages(paths) {
        let _ = app.update(message);
    }
}

#[test]
fn startup_paths_become_file_opened_messages_in_order() {
    let messages = Signex::startup_open_messages(vec!["a.snxsch".into(), "b.snxprj".into()]);
    let opened: Vec<_> = messages
        .into_iter()
        .map(|m| match m {
            Message::File(FileMsg::Opened(Some(p))) => p,
            _ => panic!("expected FileMsg::Opened(Some(_))"),
        })
        .collect();
    assert_eq!(
        opened,
        vec![
            std::path::PathBuf::from("a.snxsch"),
            std::path::PathBuf::from("b.snxprj")
        ]
    );
}

#[test]
fn project_path_argument_opens_the_project() {
    let tmp = TempDir::new().expect("tempdir");
    let prj = tmp.path().join("Demo.snxprj");
    fs::write(&prj, b"").expect("write .snxprj");

    let (mut app, _t) = Signex::new();
    assert!(app.document_state.projects.is_empty());
    dispatch_startup(&mut app, &[prj.as_path()], tmp.path());

    assert_eq!(app.document_state.projects.len(), 1);
    assert_eq!(app.document_state.projects[0].path, prj);
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn relative_project_argument_resolves_against_cwd() {
    let tmp = TempDir::new().expect("tempdir");
    fs::write(tmp.path().join("Rel.snxprj"), b"").expect("write .snxprj");

    let (mut app, _t) = Signex::new();
    dispatch_startup(&mut app, &[std::path::Path::new("Rel.snxprj")], tmp.path());

    assert_eq!(app.document_state.projects.len(), 1);
    assert_eq!(
        app.document_state.projects[0].path,
        tmp.path().join("Rel.snxprj")
    );
}

#[test]
fn unsupported_file_argument_raises_the_open_failed_notice() {
    let tmp = TempDir::new().expect("tempdir");
    let txt = tmp.path().join("notes.txt");
    fs::write(&txt, b"hi").expect("write");

    let (mut app, _t) = Signex::new();
    dispatch_startup(&mut app, &[txt.as_path()], tmp.path());

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("an unsupported file must surface an error notice");
    assert_eq!(notice.title, "Open Failed");
    assert!(notice.detail.contains("unsupported file type"));
    assert!(app.document_state.projects.is_empty());
}

#[test]
fn missing_project_argument_raises_the_open_failed_notice() {
    let tmp = TempDir::new().expect("tempdir");
    let missing = tmp.path().join("Gone.snxprj");

    let (mut app, _t) = Signex::new();
    dispatch_startup(&mut app, &[missing.as_path()], tmp.path());

    assert!(app.document_state.error_notice.is_some());
    assert!(app.document_state.projects.is_empty());
}

#[test]
fn failed_async_schematic_read_raises_the_open_failed_notice() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: "gone.snxsch".into(),
        title: "gone".into(),
        result: Err("read gone.snxsch: not found".to_string()),
    }));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert_eq!(notice.title, "Open Failed");
    assert!(notice.detail.contains("not found"));
}
