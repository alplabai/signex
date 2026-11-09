//! A half-typed parameter value that does not parse yet is unsaved work
//! (#2132): every gate that prompts, parks or enables Save All sees it, Save
//! and Save All refuse naming the field and keep the text, Discard drops it.

use signex_library::{ParamValue, RowId};

use crate::app::{
    ComponentEditorTab, Message, ProjectCloseChoice, ProjectMsg, Signex, TabInfo, TabKind,
    WindowMsg,
};
use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::{ComponentPreviewState, EditorAddress};
use crate::menu_bar::MenuMessage;

const FIELD: &str = "tcr";
const TYPED: &str = "1.2.3";

/// An app with one clean Component row open in a tab, its `tcr` number
/// parameter holding `TYPED`, which does not parse.
fn app_with_half_typed_row() -> (Signex, EditorAddress) {
    let (mut app, _task) = Signex::new();
    let lib = std::env::temp_dir()
        .join(format!("snx-2132-{}", uuid::Uuid::now_v7().simple()))
        .join("lib.snxlib");
    let mut row = crate::library::component_preview::test_row();
    row.parameters.insert(FIELD.into(), ParamValue::Number(1.0));
    let row_id = RowId(row.row_id);
    let mut editor = ComponentPreviewState::from_row(lib.clone(), "parts".into(), row);
    editor.params_edit_buf.insert(FIELD.into(), TYPED.into());
    let address = EditorAddress::new(lib.clone(), "parts".into(), row_id);
    app.library.editors.insert(address.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "U1".into(),
        path: address.synthetic_tab_path(),
        cached_document: None,
        project_id: None,
        kind: TabKind::ComponentEditor(ComponentEditorTab {
            library_path: lib,
            table: "parts".into(),
            row_id,
        }),
    });
    (app, address)
}

fn event(address: &EditorAddress, msg: EditorMsg) -> Message {
    Message::Library(LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    })
}

fn typed_text(app: &Signex, address: &EditorAddress) -> Option<String> {
    app.library
        .editors
        .get(address)
        .and_then(|editor| editor.params_edit_buf.get(FIELD).cloned())
}

fn notice_detail(app: &Signex) -> String {
    app.document_state
        .error_notice
        .as_ref()
        .map(|notice| notice.detail.clone())
        .unwrap_or_default()
}

#[test]
fn closing_the_tab_parks_the_half_typed_row_with_its_text() {
    let (mut app, address) = app_with_half_typed_row();
    assert!(!app.library.editors[&address].dirty);

    let _ = app.update(event(&address, EditorMsg::CloseEditor));

    assert!(app.document_state.tabs.is_empty(), "the tab closed");
    assert_eq!(typed_text(&app, &address).as_deref(), Some(TYPED));
}

#[test]
fn save_refuses_a_value_that_does_not_parse_names_the_field_and_keeps_the_text() {
    let (mut app, address) = app_with_half_typed_row();

    let _ = app.update(event(&address, EditorMsg::SaveDraft));

    let detail = notice_detail(&app);
    assert!(detail.contains(FIELD), "names the field: {detail}");
    assert_eq!(typed_text(&app, &address).as_deref(), Some(TYPED));
    assert!(app.library.editors[&address].saving_row.is_none());
}

#[test]
fn save_all_is_enabled_and_refuses_the_same_way() {
    let (mut app, address) = app_with_half_typed_row();
    assert!(app.command_context(None).has_dirty_documents);

    let err = app
        .begin_save_row_in_batch(&address, 1)
        .map(|_| ())
        .unwrap_err();
    assert!(format!("{err:#}").contains(FIELD), "{err:#}");
    assert_eq!(typed_text(&app, &address).as_deref(), Some(TYPED));

    let _ = app.update(Message::Menu(MenuMessage::SaveAll));
    assert_eq!(typed_text(&app, &address).as_deref(), Some(TYPED));
}

#[test]
fn the_quit_prompt_lists_the_half_typed_row_and_discard_all_drops_it() {
    let (mut app, address) = app_with_half_typed_row();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;

    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(main)));

    let confirm = app.ui_state.app_quit_confirm.as_ref().expect("prompted");
    assert_eq!(confirm.dirty_rows, vec![address.clone()]);
    assert_eq!(typed_text(&app, &address).as_deref(), Some(TYPED));

    let task = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert!(task.units() > 0, "Discard All closes the main window");
}

#[test]
fn discard_drops_the_text_of_a_parked_row() {
    let (mut app, address) = app_with_half_typed_row();
    let _ = app.update(event(&address, EditorMsg::CloseEditor));
    assert!(typed_text(&app, &address).is_some());

    let _ = app.close_component_preview(&address);

    assert!(!app.library.editors.contains_key(&address));
}
