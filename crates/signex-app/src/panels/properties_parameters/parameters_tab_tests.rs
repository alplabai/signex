//! Maintainer decision 2026-11-18: Document Options ▸ Parameters is the one
//! place a sheet's title block is edited. A Title typed there shows its value
//! in the row, is one undo step, and is written to the sheet on Save.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::Task;
use signex_types::schematic::SchematicSheet;

use crate::app::{EditMsg, FileMsg, Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::{BuiltinParameter, PanelMsg, TitleBlockField, shown_title_block_value};
use crate::persist::{DocumentSaves, DocumentSink};

#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl DocumentSink for Recorder {
    fn write(&self, _path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(bytes).into_owned());
        Ok(())
    }
}

fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Update with `message` and every message its tasks publish.
fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    for next in run(task) {
        send(app, next);
    }
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    send(app, Message::Dock(DockMessage::Panel(msg)));
}

/// An app showing one clean sheet, `power.snxsch`, with Save recorded.
fn app_on_sheet() -> (Signex, PathBuf, Arc<Recorder>) {
    let path = PathBuf::from("power.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(SchematicSheet::default()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "power".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    (app, path, recorder)
}

fn sheet_title(app: &Signex, path: &Path) -> Option<String> {
    let engine = &app.document_state.engines[path];
    engine.document().title_block.get("title").cloned()
}

fn shown_title(app: &mut Signex) -> String {
    app.refresh_panel_ctx();
    shown_title_block_value(&app.document_state.panel_ctx, TitleBlockField::Title)
}

#[test]
fn a_title_typed_in_parameters_shows_and_is_saved_to_the_sheet() {
    let (mut app, path, recorder) = app_on_sheet();
    panel(&mut app, PanelMsg::PropertiesTab(1));
    assert_eq!(shown_title(&mut app), "", "an empty field shows empty");

    panel(
        &mut app,
        PanelMsg::TitleBlockTyping(TitleBlockField::Title, "dcdc".into()),
    );
    assert_eq!(shown_title(&mut app), "dcdc", "the typed text shows");
    assert_eq!(sheet_title(&app, &path), None, "a draft is not applied");

    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Title),
    );
    assert_eq!(shown_title(&mut app), "dcdc", "the applied value shows");
    assert_eq!(sheet_title(&app, &path).as_deref(), Some("dcdc"));
    let in_panel = &app.document_state.panel_ctx.title_block;
    assert_eq!(in_panel.get("title").map(String::as_str), Some("dcdc"));

    send(&mut app, Message::Edit(EditMsg::Undo));
    let undone = sheet_title(&app, &path);
    assert_ne!(undone.as_deref(), Some("dcdc"), "one undo step");
    assert_eq!(shown_title(&mut app), "");
    send(&mut app, Message::Edit(EditMsg::Redo));
    assert_eq!(sheet_title(&app, &path).as_deref(), Some("dcdc"));

    send(&mut app, Message::File(FileMsg::Save));
    let written = recorder.0.lock().unwrap().last().cloned();
    let written = written.expect("Save wrote the sheet");
    let saved = signex_types::format::SnxSchematic::parse(&written)
        .expect("reopens")
        .sheet;
    assert_eq!(
        saved.title_block.get("title").map(String::as_str),
        Some("dcdc")
    );
}

/// The read-only rows hold what the special strings resolve to on the sheet.
#[test]
fn the_built_in_rows_hold_the_sheets_resolved_values() {
    let (mut app, _path, _recorder) = app_on_sheet();
    app.refresh_panel_ctx();
    let rows = &app.document_state.panel_ctx.builtin_parameters;
    let value = |wanted: BuiltinParameter| {
        rows.iter()
            .find(|(parameter, _)| *parameter == wanted)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(value(BuiltinParameter::DocumentName), Some("power.snxsch"));
    assert_eq!(value(BuiltinParameter::SheetName), Some("power"));
    assert_eq!(value(BuiltinParameter::ProjectName), Some(""));
}
