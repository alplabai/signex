//! Mockup row 47: text typed into a note's Text box belongs to that sheet's
//! note and reaches that sheet as one edit, through `Signex::update`, on a
//! tab switch, a tab close and a save. A copied sheet carrying the same note
//! id never receives it.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::Task;
use iced::widget::text_editor::{Action, Edit, Motion};
use signex_types::schematic::{
    HAlign, Point, SchematicSheet, SelectedItem, SelectedKind, TextNote, VAlign,
};

use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::panels::{PanelMsg, PlacedEditMsg};
use crate::persist::{DocumentSaves, DocumentSink};
use crate::tab_bar::TabMessage;

const NOTE: u128 = 47;

#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl DocumentSink for Recorder {
    fn write(&self, _path: &Path, bytes: &[u8]) -> io::Result<()> {
        let text = String::from_utf8_lossy(bytes).into_owned();
        self.0.lock().unwrap().push(text);
        Ok(())
    }
}

fn settle(app: &mut Signex, task: Task<Message>) {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let messages = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in messages {
        let _ = app.update(message);
    }
}

fn note_item() -> SelectedItem {
    SelectedItem::new(uuid::Uuid::from_u128(NOTE), SelectedKind::TextNote)
}

/// A sheet with the one note "note"; a copied sheet carries the same id.
fn sheet_with_note() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".into(),
        text_notes: vec![TextNote {
            uuid: note_item().uuid,
            text: "note".into(),
            position: Point::new(50.0, 50.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        }],
        ..Default::default()
    }
}

/// Sheets `a` and `b` open in the main window, both with the note, `a`
/// active.
fn two_sheets() -> (tempfile::TempDir, Signex, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let a = dir.path().join("a.snxsch");
    let b = dir.path().join("b.snxsch");
    add_sheet_with(&mut app, &a, sheet_with_note());
    add_sheet_with(&mut app, &b, sheet_with_note());
    switch_to(&mut app, 0);
    assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
    (dir, app, a, b)
}

fn switch_to(app: &mut Signex, index: usize) {
    let window_id = app.ui_state.main_window_id.expect("main window");
    let msg = TabMessage::Select(index);
    let _ = app.update(Message::Tab { window_id, msg });
}

fn panel(app: &mut Signex, msg: PlacedEditMsg) {
    let msg = PanelMsg::PlacedEdit(msg);
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

/// Select the note on the active sheet and type `c` at the end of its text,
/// without Ctrl+Enter.
fn type_into_note(app: &mut Signex, c: char) {
    let item = note_item();
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app.update_selection_info();
    let _ = app.update(Message::Ui(crate::app::UiMsg::Noop));
    for action in [
        Action::Move(Motion::DocumentEnd),
        Action::Edit(Edit::Insert(c)),
    ] {
        panel(app, PlacedEditMsg::NoteActed { item, action });
    }
}

fn note_text(app: &Signex, sheet: &Path) -> String {
    app.document_state.engines[sheet].document().text_notes[0]
        .text
        .clone()
}

#[test]
fn a_tab_switch_applies_the_typed_note_to_its_own_sheet() {
    let (_dir, mut app, a, b) = two_sheets();
    type_into_note(&mut app, '!');
    assert_eq!(note_text(&app, &a), "note", "still a draft");
    switch_to(&mut app, 1);
    assert_eq!(note_text(&app, &a), "note!");
    assert!(
        app.document_state.is_document_dirty(&a),
        "the tab reads dirty"
    );
    assert_eq!(note_text(&app, &b), "note", "b carries the same id");
    assert!(!app.document_state.is_document_dirty(&b));
}

/// The same note id on two sheets: what is typed on each stays on it.
#[test]
fn the_same_note_id_on_two_sheets_keeps_each_sheets_text() {
    let (_dir, mut app, a, b) = two_sheets();
    type_into_note(&mut app, '!');
    switch_to(&mut app, 1);
    type_into_note(&mut app, '?');
    switch_to(&mut app, 0);
    assert_eq!(note_text(&app, &a), "note!");
    assert_eq!(note_text(&app, &b), "note?");
}

/// Closing the tab applies the typed text first, so the sheet parks dirty
/// and asks to be saved like any other edit.
#[test]
fn a_tab_close_applies_the_typed_note_and_parks_the_sheet_dirty() {
    let (_dir, mut app, a, _b) = two_sheets();
    type_into_note(&mut app, '!');
    let window_id = app.ui_state.main_window_id.expect("main window");
    let msg = TabMessage::CloseClicked(0);
    let task = app.update(Message::Tab { window_id, msg });
    settle(&mut app, task);
    assert_eq!(note_text(&app, &a), "note!");
    assert!(app.document_state.is_document_dirty(&a));
}

/// Ctrl+S while the box is still being typed in writes the typed text.
#[test]
fn a_save_while_typing_writes_the_typed_note() {
    let (_dir, mut app, a, _b) = two_sheets();
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    type_into_note(&mut app, '!');
    let task = app.update(Message::Menu(MenuMessage::command("save_document")));
    settle(&mut app, task);
    let written = recorder.0.lock().unwrap().last().cloned().expect("a write");
    let sheet = signex_types::format::SnxSchematic::parse(&written)
        .expect("reopens")
        .sheet;
    assert_eq!(sheet.text_notes[0].text, "note!");
    assert_eq!(note_text(&app, &a), "note!");
    let draft = app.document_state.panel_ctx.placed.note.as_ref();
    assert!(draft.is_some_and(|d| !d.edited), "the box is applied once");
}

/// Project `ProjectA` with its sheet (the note on it) open and active, and
/// "!" typed into the note's box without Ctrl+Enter.
fn project_with_typed_note() -> (Signex, PathBuf, crate::app::ProjectId) {
    use crate::app::handlers::menu::export::tests as fx;
    let mut app = fx::app_workspace("/w/p", &["top.snxsch"]);
    let id = app.document_state.projects[0].id;
    let sheet = PathBuf::from("/w/p/top.snxsch");
    fx::open_with(&mut app.document_state, &sheet, sheet_with_note());
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "top".to_string(),
        path: sheet.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(sheet.clone());
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder);
    type_into_note(&mut app, '!');
    assert_eq!(note_text(&app, &sheet), "note", "still a draft");
    (app, sheet, id)
}

/// Close Project while the note box is typed in: the text reaches the sheet
/// before the gate reads the dirty marks, so the gate asks instead of
/// closing and dropping it.
#[test]
fn close_project_while_typing_a_note_commits_it_and_prompts() {
    use crate::app::{ContextMenuMsg, ProjectMsg, ProjectTreeAction};
    let (mut app, sheet, id) = project_with_typed_note();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![id.to_string()]),
    )));
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id: id,
        result,
    }));
    assert_eq!(note_text(&app, &sheet), "note!", "nothing is lost");
    assert!(app.document_state.is_document_dirty(&sheet));
    assert!(
        app.ui_state.project_close_confirm.is_some(),
        "the typed note is an unsaved edit"
    );
    assert!(app.document_state.projects.iter().any(|p| p.id == id));
}

/// Project ▸ Close Project (the active project's route) commits it too.
#[test]
fn close_active_project_while_typing_a_note_commits_it() {
    let (mut app, sheet, _id) = project_with_typed_note();
    let _ = app.close_active_project();
    assert_eq!(note_text(&app, &sheet), "note!");
    assert!(app.document_state.is_document_dirty(&sheet));
}

/// An export reads the sheet with the text typed into the note box.
#[test]
fn an_export_while_typing_a_note_reads_the_typed_text() {
    let (mut app, sheet, _id) = project_with_typed_note();
    let _ = app.handle_bom_preview_open();
    assert_eq!(note_text(&app, &sheet), "note!");
}

/// A project in a temp directory, its sheet (the note on it) open and
/// active with "!" typed into the note's box, and the Remove dialog open
/// on that sheet.
fn remove_dialog_on_typed_note() -> (Signex, PathBuf, tempfile::TempDir) {
    use crate::app::RemoveDialogState;
    use crate::app::handlers::menu::export::tests as fx;
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = fx::app_workspace(dir.path().to_str().expect("utf8"), &["top.snxsch"]);
    let id = app.document_state.projects[0].id;
    let sheet = dir.path().join("top.snxsch");
    let engine = signex_engine::Engine::new_with_path(sheet_with_note(), Some(sheet.clone()))
        .expect("engine");
    app.document_state.engines.insert(sheet.clone(), engine);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "top".to_string(),
        path: sheet.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(sheet.clone());
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder);
    type_into_note(&mut app, '!');
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: sheet.clone(),
        tree_path: vec![0],
        project_id: Some(id),
        display_name: "top".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
        is_directory_package: false,
    });
    (app, sheet, dir)
}

/// Exclude from Project on a sheet whose only change is a typed note asks
/// first; the sheet stays open and the text is still in the box.
#[test]
fn exclude_on_a_sheet_with_only_a_typed_note_asks_first() {
    let (mut app, sheet, _dir) = remove_dialog_on_typed_note();
    let _ = app.handle_remove_confirm(crate::app::RemoveChoice::ExcludeFromProject);
    let dialog = app.ui_state.remove_dialog.as_ref().expect("dialog stays");
    assert!(dialog.unsaved_prompt, "the typed note is an unsaved edit");
    assert!(
        app.document_state.engines.contains_key(&sheet),
        "still open"
    );
    let draft = app.document_state.panel_ctx.placed.note.as_ref();
    assert!(draft.is_some_and(|d| d.edited), "the text waits in the box");
}

/// Save on that prompt writes the typed text.
#[test]
fn save_on_the_exclude_prompt_writes_the_typed_note() {
    use crate::app::{ExcludeDirtyChoice, RemoveChoice, RemoveMsg};
    let (mut app, sheet, _dir) = remove_dialog_on_typed_note();
    let _ = app.handle_remove_confirm(RemoveChoice::ExcludeFromProject);
    let save = Message::Remove(RemoveMsg::ExcludeDirty(ExcludeDirtyChoice::Save));
    let task = app.update(save);
    settle(&mut app, task);
    let on_disk = std::fs::read_to_string(&sheet).expect("the sheet was written");
    let saved = signex_types::format::SnxSchematic::parse(&on_disk)
        .expect("parses")
        .sheet;
    assert_eq!(saved.text_notes[0].text, "note!");
}

/// Discard on the Exclude prompt throws the typed text away with the
/// sheet's other edits: the file keeps its old text, no typed box is left
/// in any Properties context, and the next update has nothing owed to
/// apply (it would warn that the note is gone).
#[test]
fn discard_on_the_exclude_prompt_drops_the_typed_note() {
    use crate::app::{ExcludeDirtyChoice, RemoveChoice, RemoveMsg};
    let (mut app, sheet, _dir) = remove_dialog_on_typed_note();
    let on_file = signex_types::format::SnxSchematic::new(sheet_with_note())
        .write_string()
        .expect("encodes");
    std::fs::write(&sheet, on_file).expect("the sheet is on disk");
    let _ = app.handle_remove_confirm(RemoveChoice::ExcludeFromProject);
    let discard = Message::Remove(RemoveMsg::ExcludeDirty(ExcludeDirtyChoice::Discard));
    let task = app.update(discard);
    settle(&mut app, task);

    let on_disk = std::fs::read_to_string(&sheet).expect("still on disk");
    let read = signex_types::format::SnxSchematic::parse(&on_disk)
        .expect("parses")
        .sheet;
    assert_eq!(read.text_notes[0].text, "note", "the old text stays");

    let docs = &app.document_state;
    let panels = std::iter::once(&docs.panel_ctx.placed)
        .chain(docs.window_panel_ctx.values().map(|ctx| &ctx.placed));
    for placed in panels {
        assert!(placed.notes_owed.is_empty(), "{:?}", placed.notes_owed);
        assert!(
            placed.note.as_ref().is_none_or(|d| !d.edited),
            "no typed box is left"
        );
    }

    let gone = "the note is no longer there";
    let (said, gone_before) = (
        crate::diagnostics::said_on_this_thread(),
        crate::diagnostics::logged_count(gone),
    );
    let _ = app.update(Message::Ui(crate::app::UiMsg::Noop));
    assert_eq!(
        crate::diagnostics::said_on_this_thread(),
        said,
        "the next update logs nothing"
    );
    assert_eq!(crate::diagnostics::logged_count(gone), gone_before);
}
