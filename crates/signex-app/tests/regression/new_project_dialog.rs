//! #2232 — every New Project entry point opens the setup dialog, and the
//! dialog writes nothing until Create.

use signex_app::app::{Message, NewProjectMsg, Signex};
use signex_app::menu_bar::MenuMessage;
use signex_app::start_screen::Action;

fn opens_the_dialog(message: Message) {
    let (mut app, _t) = Signex::new();
    assert!(app.ui_state.new_project.is_none());
    crate::settle_support::settle(&mut app, message);
    assert!(app.ui_state.new_project.is_some(), "the dialog is open");
    assert!(
        app.document_state.projects.is_empty(),
        "nothing created yet"
    );
    crate::settle_support::settle(&mut app, Message::from(NewProjectMsg::CancelRequested));
    assert!(app.ui_state.new_project.is_none(), "Cancel closes it");
}

#[test]
fn the_file_menu_row_opens_the_setup_dialog() {
    opens_the_dialog(crate::menu_support::menu_row("new_document"));
}

#[test]
fn the_start_screen_button_opens_the_setup_dialog() {
    opens_the_dialog(Action::NewProject.message());
}

#[test]
fn the_palette_and_keymap_command_opens_the_setup_dialog() {
    // The palette row and Ctrl+N run `new_document` through the same bridge;
    // the profile command without operands opens the dialog as well.
    opens_the_dialog(crate::menu_support::menu_row("new_project_with_profile"));
    opens_the_dialog(Message::Menu(MenuMessage::NewProject));
}

#[test]
fn create_from_the_dialog_opens_the_new_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, _t) = Signex::new();
    crate::settle_support::settle(&mut app, Action::NewProject.message());
    for msg in [
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
        NewProjectMsg::NameEdited("Dialog Made".into()),
        NewProjectMsg::SubmitRequested,
    ] {
        crate::settle_support::settle(&mut app, Message::from(msg));
    }
    let project = tmp.path().join("Dialog Made").join("Dialog Made.snxprj");
    assert!(project.is_file());
    assert_eq!(app.document_state.projects.len(), 1, "the project is open");
    let data = &app.document_state.projects[0].data;
    assert_eq!(
        data.project_defaults.pcb.as_ref().map(|p| p.copper_layers),
        Some(4),
        "Standard 4-layer by default"
    );
}
