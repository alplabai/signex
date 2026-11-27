//! #3194: a drawing value still being typed in Properties is only a preview
//! in the engine. Save, Save All, an export and Close Project + Save All must
//! settle it first, so nothing they write or read is a value with no undo
//! step behind it.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use iced::Task;
use signex_types::schematic::SelectedKind;
use signex_types::schematic::{FillType, Point, SchDrawing, SchematicSheet, SelectedItem};

use crate::app::{EditMsg, ExportMsg, Message, ProjectId, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::panels::{DrawingFieldId as F, PanelMsg};
use crate::persist::{DocumentSaves, DocumentSink};

/// Every `.snxsch` write, in order.
#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl DocumentSink for Recorder {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if path.extension().is_some_and(|ext| ext == "snxsch") {
            let text = String::from_utf8_lossy(bytes).into_owned();
            self.0.lock().unwrap().push(text);
        }
        Ok(())
    }
}

/// Run `task` and every task its messages return, as the runtime does.
fn settle(app: &mut Signex, task: Task<Message>) {
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("settled");
}

const SAVED_WIDTH: f64 = 20.0;

/// Project `ProjectA` with its one sheet open and saved, a rectangle of
/// width 20 selected, "45" typed into its Width box and not left: a preview.
fn typing_in_project() -> (Signex, PathBuf, ProjectId, Arc<Recorder>) {
    use crate::app::handlers::menu::export::tests as fx;
    let mut app = fx::app_workspace("/w/p", &["top.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    let id = app.document_state.projects[0].id;
    let sheet = PathBuf::from("/w/p/top.snxsch");
    let rect = SchDrawing::Rect {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.0, 10.0),
        end: Point::new(10.0 + SAVED_WIDTH, 20.0),
        width: 0.2,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    };
    let rect_id = rect.uuid();
    let doc = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        drawings: vec![rect],
        ..Default::default()
    };
    fx::open_with(&mut app.document_state, &sheet, doc);
    app.document_state
        .engines
        .get_mut(&sheet)
        .expect("engine")
        .mark_saved();
    app.document_state.tabs.push(TabInfo {
        title: "top".to_string(),
        path: sheet.clone(),
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(sheet.clone());
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(rect_id, SelectedKind::Drawing)];
    app.update_selection_info();
    type_width(&mut app, "45");
    assert_eq!(width_in_engine(&app, &sheet), 45.0, "the preview is shown");
    assert!(!app.document_state.is_document_dirty(&sheet));
    assert!(!app.document_state.engines[&sheet].can_undo());
    (app, sheet, id, recorder)
}

fn type_width(app: &mut Signex, text: &str) {
    let msg = PanelMsg::DrawingFieldTyping(F::RectWidth, text.into());
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn rect_width(sheet: &SchematicSheet) -> f64 {
    match &sheet.drawings[0] {
        SchDrawing::Rect { start, end, .. } => (end.x - start.x).abs(),
        other => panic!("not a rectangle: {other:?}"),
    }
}

fn width_in_engine(app: &Signex, sheet: &Path) -> f64 {
    rect_width(app.document_state.engines[sheet].document())
}

fn written_width(recorder: &Recorder) -> f64 {
    let bytes = recorder.0.lock().unwrap().last().cloned().expect("a write");
    let saved = signex_types::format::SnxSchematic::parse(&bytes).expect("reopens");
    rect_width(&saved.sheet)
}

/// The preview is no longer open in the engine and one undo takes the sheet
/// back to the value from before the first keystroke.
fn assert_settled_as_one_step(app: &mut Signex, sheet: &Path) {
    let engine = &app.document_state.engines[sheet];
    assert_eq!(engine.live_edit_drawing(), None, "the engine settled it");
    assert!(app.document_view.live_drawing_edit.is_none());
    assert!(engine.can_undo(), "an undo step covers the typed value");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(width_in_engine(app, sheet), SAVED_WIDTH);
    assert!(!app.document_state.engines[sheet].can_undo(), "one step");
}

#[test]
fn save_writes_the_typed_value_as_a_committed_step() {
    let (mut app, sheet, _id, recorder) = typing_in_project();
    let task = app.update(Message::Menu(MenuMessage::command("save_document")));
    settle(&mut app, task);
    assert_eq!(written_width(&recorder), 45.0);
    assert!(!app.document_state.is_document_dirty(&sheet), "saved");
    assert_settled_as_one_step(&mut app, &sheet);
    assert!(
        app.document_state.is_document_dirty(&sheet),
        "the file holds 45, the sheet 20 again"
    );
}

/// Text the box cannot store was never a value: the save takes the preview
/// back and writes the value from before the edit.
#[test]
fn save_on_a_typo_writes_the_value_from_before_the_edit() {
    let (mut app, sheet, _id, recorder) = typing_in_project();
    type_width(&mut app, "45x");
    let task = app.update(Message::Menu(MenuMessage::command("save_document")));
    settle(&mut app, task);
    let written = recorder.0.lock().unwrap().last().cloned();
    if let Some(bytes) = written {
        let saved = signex_types::format::SnxSchematic::parse(&bytes).expect("reopens");
        assert_eq!(rect_width(&saved.sheet), SAVED_WIDTH);
    }
    assert_eq!(width_in_engine(&app, &sheet), SAVED_WIDTH);
    assert_eq!(app.document_state.engines[&sheet].live_edit_drawing(), None);
    assert!(!app.document_state.engines[&sheet].can_undo());
}

#[test]
fn save_all_writes_the_typed_value_as_a_committed_step() {
    let (mut app, sheet, _id, recorder) = typing_in_project();
    let task = app.update(Message::Menu(MenuMessage::command("save_all")));
    assert_eq!(app.document_state.engines[&sheet].live_edit_drawing(), None);
    assert!(app.document_state.is_document_dirty(&sheet), "committed");
    settle(&mut app, task);
    assert!(
        !recorder.0.lock().unwrap().is_empty(),
        "Save All wrote nothing; dirty {}",
        app.document_state.is_document_dirty(&sheet)
    );
    assert_eq!(written_width(&recorder), 45.0);
    assert!(!app.document_state.is_document_dirty(&sheet), "saved");
    assert_settled_as_one_step(&mut app, &sheet);
}

/// The export reads the engine's document; it must read a committed one.
#[test]
fn netlist_export_settles_the_preview_before_it_reads() {
    let (mut app, sheet, _id, _recorder) = typing_in_project();
    drop(app.update(Message::Menu(MenuMessage::ExportNetlist)));
    assert_eq!(width_in_engine(&app, &sheet), 45.0);
    assert!(app.document_state.is_document_dirty(&sheet));
    assert_settled_as_one_step(&mut app, &sheet);
}

#[test]
fn pdf_export_settles_the_preview_before_it_reads() {
    let (mut app, sheet, _id, _recorder) = typing_in_project();
    drop(app.update(Message::Export(ExportMsg::PdfOpenDialog)));
    assert_eq!(width_in_engine(&app, &sheet), 45.0);
    assert!(app.document_state.is_document_dirty(&sheet));
    assert_settled_as_one_step(&mut app, &sheet);
}

/// Close Project reads the dirty marks at its gate: a sheet whose only change
/// is the preview must prompt, and Save All must write the typed value.
#[test]
fn close_project_then_save_all_writes_the_typed_value() {
    use crate::app::{ContextMenuMsg, ProjectCloseChoice, ProjectMsg, ProjectTreeAction};
    let (mut app, sheet, id, recorder) = typing_in_project();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![id.to_string()]),
    )));
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id: id,
        result,
    }));
    assert!(
        app.ui_state.project_close_confirm.is_some(),
        "the preview is an unsaved edit at the gate; instead: projects open {}, \
         engine kept {}, writes {}",
        app.document_state.projects.len(),
        app.document_state.engines.contains_key(&sheet),
        recorder.0.lock().unwrap().len(),
    );
    assert!(app.document_state.is_document_dirty(&sheet));
    assert_eq!(app.document_state.engines[&sheet].live_edit_drawing(), None);
    let task = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::SaveAll,
    )));
    settle(&mut app, task);
    assert_eq!(written_width(&recorder), 45.0);
}

/// Closing the sheet's tab parks the typed value as an edit; Save All then
/// writes it.
#[test]
fn closing_the_tab_then_save_all_writes_the_typed_value() {
    let (mut app, sheet, _id, recorder) = typing_in_project();
    let id = "close_active_document";
    let _ = app.update(Message::Menu(MenuMessage::command(id)));
    assert_eq!(width_in_engine(&app, &sheet), 45.0);
    assert!(app.document_state.is_document_dirty(&sheet), "parked dirty");
    assert_eq!(app.document_state.engines[&sheet].live_edit_drawing(), None);
    let task = app.update(Message::Menu(MenuMessage::command("save_all")));
    settle(&mut app, task);
    assert_eq!(written_width(&recorder), 45.0);
}

/// Quit counts a sheet whose only change is the preview as unsaved.
#[test]
fn the_quit_prompt_counts_a_preview_only_sheet() {
    let (mut app, sheet, _id, _recorder) = typing_in_project();
    let _ = app.handle_app_quit_requested();
    let confirm = app.ui_state.app_quit_confirm.as_ref().expect("a prompt");
    assert!(
        confirm.dirty_paths.contains(&sheet),
        "{:?}",
        confirm.dirty_paths
    );
}
