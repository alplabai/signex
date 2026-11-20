//! The Rename dialog (#23): the sheet symbols it names, the name clash it
//! shows at once and the focused field it opens with.

use signex_widgets::tree_view::TreeKey;

use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
use crate::app::runtime::sheet_tree_tests::{settle, write_sheet};
use crate::app::{Message, ProjectTreeAction, RenameMsg, Signex};

fn row_key(app: &Signex, path: &[&str]) -> TreeKey {
    let mut key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
    ];
    key.extend(path.iter().map(|s| s.to_string()));
    key
}

/// A project of `top.snxsch` placing `child.snxsch` twice, as "U_power" and
/// "U_aux", both on disk and walked.
fn hierarchy(dir: &std::path::Path) -> Signex {
    let mut top = sheet_with_net("R_TOP", "N", &["child.snxsch", "child.snxsch"]);
    for (symbol, name) in top.child_sheets.iter_mut().zip(["U_power", "U_aux"]) {
        symbol.name = name.to_string();
        symbol.uuid = uuid::Uuid::now_v7();
    }
    let text = signex_types::format::SnxSchematic::new(top)
        .write_string()
        .expect("encode");
    std::fs::write(dir.join("top.snxsch"), text).expect("write top");
    write_sheet(dir, "child.snxsch", &[]);
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch", "child.snxsch"]);
    settle(&mut app, false);
    app
}

fn open(app: &mut Signex, path: &[&str]) -> iced::Task<Message> {
    let key = row_key(app, path);
    app.handle_project_tree_action(ProjectTreeAction::OpenRenameDialog(key))
}

/// Renaming a placed sheet lists the symbols of its own project that place
/// it (the ones the rename re-points); a sheet nobody places lists none.
#[test]
fn the_dialog_names_the_sheet_symbols_that_place_the_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = hierarchy(tmp.path());

    let _ = open(&mut app, &["top.snxsch", "child.snxsch"]);
    let dialog = app.ui_state.rename_dialog.clone().expect("dialog opens");
    let mut names: Vec<&str> = dialog
        .placing_symbols
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["U_aux", "U_power"], "{:?}", dialog.placing_symbols);
    assert!(
        dialog
            .placing_symbols
            .iter()
            .all(|s| s.parent == tmp.path().join("top.snxsch")),
        "{:?}",
        dialog.placing_symbols
    );

    app.ui_state.rename_dialog = None;
    let _ = open(&mut app, &["top.snxsch"]);
    let dialog = app.ui_state.rename_dialog.clone().expect("dialog opens");
    assert!(
        dialog.placing_symbols.is_empty(),
        "nobody places the top sheet"
    );
}

/// The field starts with the whole current name. Opening starts no work;
/// once the dialog's window exists its name field is focused and selected,
/// and no other dialog's window does that.
#[test]
fn the_name_field_is_focused_once_the_window_opens() {
    use crate::app::state::ModalId;
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = hierarchy(tmp.path());
    assert_eq!(
        app.focus_rename_field_once_open(ModalId::RenameDialog)
            .units(),
        0,
        "no dialog, nothing to focus"
    );
    let opened = open(&mut app, &["top.snxsch"]);
    assert_eq!(opened.units(), 0, "opening starts no work");
    let dialog = app.ui_state.rename_dialog.as_ref().expect("dialog opens");
    assert_eq!(dialog.buffer, "top.snxsch");
    assert!(dialog.name_clash.is_none());
    assert_ne!(
        app.focus_rename_field_once_open(ModalId::RenameDialog)
            .units(),
        0,
        "the focus and select-all task"
    );
    assert_eq!(
        app.focus_rename_field_once_open(ModalId::RemoveDialog)
            .units(),
        0
    );
}

/// A name another file of the folder has is shown as soon as it is typed and
/// Rename is refused while it stands, Enter included; nothing moves. A new
/// name, or the file's own, clears it.
#[test]
fn a_name_the_folder_already_has_is_shown_and_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = hierarchy(tmp.path());
    let _ = open(&mut app, &["top.snxsch", "child.snxsch"]);

    let _ = app.update(Message::Rename(RenameMsg::BufferChanged(
        "top.snxsch".to_string(),
    )));
    let dialog = app.ui_state.rename_dialog.as_ref().expect("dialog");
    assert_eq!(dialog.name_clash.as_deref(), Some("top.snxsch"));

    // Submit goes through update; the units are counted on the handler,
    // because update() also carries the batched background tasks.
    let _ = app.update(Message::Rename(RenameMsg::Submit));
    let submitted = app.handle_rename_submit();
    assert_eq!(submitted.units(), 0, "no move starts");
    assert!(
        app.ui_state.rename_dialog.is_some(),
        "the dialog stays open"
    );
    assert!(tmp.path().join("child.snxsch").exists());
    assert!(
        app.document_state
            .pending_path_ops
            .blocking_write(&tmp.path().join("child.snxsch"))
            .is_none(),
        "no rename is registered"
    );

    for free in ["power_stage.snxsch", "child.snxsch"] {
        let _ = app.update(Message::Rename(RenameMsg::BufferChanged(free.to_string())));
        let dialog = app.ui_state.rename_dialog.as_ref().expect("dialog");
        assert!(dialog.name_clash.is_none(), "{free}");
    }
}

/// An edit clears the last refusal and keeps the whole name the field
/// composed (stem plus the fixed extension).
#[test]
fn an_edit_keeps_the_composed_name_and_clears_the_last_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = hierarchy(tmp.path());
    let _ = open(&mut app, &["top.snxsch", "child.snxsch"]);
    app.set_rename_error("Name cannot be empty.");
    let _ = app.update(Message::Rename(RenameMsg::BufferChanged(
        "power_stage.snxsch".to_string(),
    )));
    let dialog = app.ui_state.rename_dialog.as_ref().expect("dialog");
    assert_eq!(dialog.buffer, "power_stage.snxsch");
    assert!(dialog.error.is_none());
}

/// A project rename has no clash check here: its own checks run on submit.
#[test]
fn a_project_rename_is_not_checked_against_the_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = hierarchy(tmp.path());
    app.open_project_rename_dialog(vec![0]);
    let _ = app.update(Message::Rename(RenameMsg::BufferChanged("top".to_string())));
    let dialog = app.ui_state.rename_dialog.as_ref().expect("dialog");
    assert!(dialog.is_project_rename);
    assert!(dialog.name_clash.is_none());
    assert!(dialog.placing_symbols.is_empty());
}
