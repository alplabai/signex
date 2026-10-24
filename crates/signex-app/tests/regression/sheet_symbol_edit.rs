//! #794 / #876 — editing a placed sheet symbol's Name and File from the
//! Properties panel, and the Place Sheet default stem skipping files that
//! exist on disk. Driven through the real message path (panel messages,
//! Undo / Redo), against real temp directories.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use tempfile::TempDir;

use super::place_sheet_tool::{app_with_sheet_tool, drag, sheets};

/// An app with one placed sheet symbol (`sheet1` / `sheet1.snxsch`) in a
/// saved-path parent, plus the sheet's id and the parent's path.
fn app_with_one_sheet(tmp: &TempDir) -> (Signex, uuid::Uuid, std::path::PathBuf) {
    let path = tmp.path().join("root.snxsch");
    let mut app = app_with_sheet_tool(path.clone());
    drag(&mut app, (10.0, 10.0), (50.0, 40.0));
    let id = sheets(&app)[0].uuid;
    app.document_state.dirty_paths.clear();
    (app, id, path)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn type_and_commit_name(app: &mut Signex, id: uuid::Uuid, text: &str) {
    panel(app, PanelMsg::ChildSheetNameTyping(id, text.to_string()));
    panel(app, PanelMsg::CommitChildSheetName(id));
}

fn type_and_commit_file(app: &mut Signex, id: uuid::Uuid, text: &str) {
    panel(app, PanelMsg::ChildSheetFileTyping(id, text.to_string()));
    panel(app, PanelMsg::CommitChildSheetFile(id));
}

fn identity(app: &Signex) -> (String, String) {
    let s = &sheets(app)[0];
    (s.name.clone(), s.filename.clone())
}

fn original() -> (String, String) {
    ("sheet1".to_string(), "sheet1.snxsch".to_string())
}

#[test]
fn editing_the_name_is_dirty_undoable_and_redoable() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, id, path) = app_with_one_sheet(&tmp);

    type_and_commit_name(&mut app, id, "Power");

    assert_eq!(identity(&app).0, "Power");
    assert_eq!(identity(&app).1, "sheet1.snxsch", "file is untouched");
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.error_notice.is_none());

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(identity(&app), original());
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(identity(&app).0, "Power");
}

#[test]
fn editing_the_file_repoints_without_touching_the_disk() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, id, path) = app_with_one_sheet(&tmp);

    type_and_commit_file(&mut app, id, "sub/power.snxsch");

    assert_eq!(identity(&app).1, "sub/power.snxsch");
    assert_eq!(identity(&app).0, "sheet1", "name is untouched");
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.error_notice.is_none());
    assert!(!tmp.path().join("sub").exists(), "no folder is created");
    assert!(!tmp.path().join("sub/power.snxsch").exists());
    assert!(!tmp.path().join("sheet1.snxsch").exists());

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(identity(&app), original());
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(identity(&app).1, "sub/power.snxsch");
}

#[test]
fn an_invalid_filename_is_refused_with_a_visible_message() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, id, path) = app_with_one_sheet(&tmp);

    for bad in ["", "   ", "power.txt", "power", ".snxsch"] {
        app.document_state.error_notice = None;
        type_and_commit_file(&mut app, id, bad);
        let notice = app
            .document_state
            .error_notice
            .as_ref()
            .unwrap_or_else(|| panic!("'{bad}' must raise a visible notice"));
        assert!(!notice.detail.is_empty());
        assert_eq!(identity(&app), original(), "'{bad}' must not apply");
    }
    assert!(!app.document_state.dirty_paths.contains(&path));
}

#[test]
fn an_empty_or_duplicate_name_is_refused_with_a_visible_message() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, id, _) = app_with_one_sheet(&tmp);
    drag(&mut app, (60.0, 10.0), (100.0, 40.0));
    assert_eq!(sheets(&app)[1].name, "sheet2");

    for bad in ["  ", "SHEET2"] {
        app.document_state.error_notice = None;
        type_and_commit_name(&mut app, id, bad);
        assert!(app.document_state.error_notice.is_some(), "'{bad}'");
        assert_eq!(identity(&app), original());
    }
}

#[test]
fn a_path_escaping_the_sheets_folder_is_refused() {
    let tmp = TempDir::new().expect("tempdir");
    let outside = TempDir::new().expect("tempdir");
    let (mut app, id, path) = app_with_one_sheet(&tmp);
    let absolute = outside
        .path()
        .join("x.snxsch")
        .to_string_lossy()
        .into_owned();

    for bad in ["../evil.snxsch", "sub/../../evil.snxsch", absolute.as_str()] {
        app.document_state.error_notice = None;
        type_and_commit_file(&mut app, id, bad);
        let notice = app
            .document_state
            .error_notice
            .as_ref()
            .unwrap_or_else(|| panic!("'{bad}' must raise a visible notice"));
        assert!(notice.detail.contains("folder"), "{notice:?}");
        assert_eq!(identity(&app), original(), "'{bad}' must not apply");
    }
    assert!(!app.document_state.dirty_paths.contains(&path));
}

#[test]
fn committing_without_typing_changes_nothing() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, id, path) = app_with_one_sheet(&tmp);

    panel(&mut app, PanelMsg::CommitChildSheetName(id));
    panel(&mut app, PanelMsg::CommitChildSheetFile(id));

    assert_eq!(identity(&app), original());
    assert!(app.document_state.error_notice.is_none());
    assert!(!app.document_state.dirty_paths.contains(&path));
}

#[test]
fn the_default_stem_skips_an_unlisted_sheet_already_on_disk() {
    let tmp = TempDir::new().expect("tempdir");
    // Sits in the folder but is listed nowhere (no project, no placed symbol).
    std::fs::write(tmp.path().join("sheet1.snxsch"), "stale").expect("write");
    let mut app = app_with_sheet_tool(tmp.path().join("root.snxsch"));

    drag(&mut app, (10.0, 10.0), (50.0, 40.0));

    let placed = sheets(&app);
    assert_eq!(placed[0].name, "sheet2");
    assert_eq!(placed[0].filename, "sheet2.snxsch");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("sheet1.snxsch")).expect("read"),
        "stale",
        "the existing file must be left alone"
    );
}
