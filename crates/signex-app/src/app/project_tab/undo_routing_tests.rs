//! #1202 / #1209 — with the ERC dialog open over a Project tab, Ctrl+Z follows
//! the window it came from. Driven through `dispatch_command`, the keyboard's
//! entry point; `ErcMsg::RulesUndone` / `EditMsg::Undo` sent directly would
//! skip the routing under test.

use signex_erc::{RuleKind, Severity};

use crate::app::command::CommandArgs;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::{ModalId, WindowKind};
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::keymap::AppCommandId;

fn project_tab_with_stored_rules() -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-prj-undo-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    // Store seeds from machine preferences other tests write.
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app.open_project_tab(0);
    let _ = app.handle_store_erc_rules_in_project();
    app
}

fn press(app: &mut Signex, window: Option<iced::window::Id>) {
    let id = AppCommandId::new("undo").expect("catalog id");
    let _ = app.dispatch_command(&id, CommandArgs::none(), window);
}

#[test]
fn undo_follows_the_window_it_came_from_with_the_erc_dialog_over_a_project_tab() {
    let mut app = project_tab_with_stored_rules();
    let project_id = app.document_state.projects[0].id;
    let seeded = app.document_state.projects[0].data.rules.clone();
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::DescriptionEdited {
            project_id,
            text: "kept".to_string(),
        },
    )));
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    assert_ne!(app.document_state.projects[0].data.rules, seeded);

    app.ui_state.erc_dialog_open = true;
    let erc = iced::window::Id::unique();
    app.ui_state
        .windows
        .insert(erc, WindowKind::DetachedModal(ModalId::ErcDialog));
    let main = app.ui_state.main_window_id;

    // The ERC window's Ctrl+Z steps the rule history, not the description.
    press(&mut app, Some(erc));
    let data = &app.document_state.projects[0].data;
    assert_eq!(data.rules, seeded, "the ERC window steps the rules");
    assert_eq!(data.description.as_deref(), Some("kept"));

    // The main window's Ctrl+Z undoes the Project tab and leaves the rules.
    press(&mut app, main);
    let data = &app.document_state.projects[0].data;
    assert_eq!(data.description, None, "the main window undoes the tab");
    assert_eq!(data.rules, seeded, "the hidden rule history is untouched");
}
