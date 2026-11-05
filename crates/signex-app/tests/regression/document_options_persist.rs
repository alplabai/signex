//! #2027 - every Document Options control that is stored in the `.snxsch`
//! goes through an undoable engine command, dirties the sheet, and is in the
//! bytes a Save writes: change, check dirty, save, reopen, value kept.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::Task;
use signex_app::app::{EditMsg, FileMsg, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, PaperEdge, SheetFrameEdit, TitleBlockField};
use signex_app::persist::{DocumentSaves, DocumentSink};
use signex_types::format::SnxSchematic;
use signex_types::schematic::{SchematicSheet, SheetOrientation};

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

fn clean_app() -> (Signex, PathBuf, Arc<Recorder>) {
    let path = PathBuf::from("doc-options.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let recorder = Arc::new(Recorder::default());
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "doc-options".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    (app, path, recorder)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
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

/// Change a control, require the dirty mark, Save, and reopen the bytes.
fn change_save_reopen(change: impl FnOnce(&mut Signex)) -> SchematicSheet {
    let (mut app, path, recorder) = clean_app();
    assert!(!app.document_state.is_document_dirty(&path), "starts clean");
    change(&mut app);
    assert!(
        app.document_state.is_document_dirty(&path),
        "the change dirties the sheet"
    );
    let task = app.update(Message::File(FileMsg::Save));
    for message in run(task) {
        let _ = app.update(message);
    }
    let written = recorder.0.lock().unwrap().last().cloned();
    let written = written.expect("Save wrote the sheet");
    SnxSchematic::parse(&written).expect("reopens").sheet
}

#[test]
fn the_paper_size_is_saved() {
    let sheet = change_save_reopen(|app| panel(app, PanelMsg::SetPaperSize("A3".into())));
    assert_eq!(sheet.paper_size, "A3");
}

#[test]
fn the_orientation_is_saved() {
    let sheet = change_save_reopen(|app| {
        panel(
            app,
            PanelMsg::SetSheetOrientation(SheetOrientation::Portrait),
        )
    });
    assert_ne!(sheet.paper_size, "A4", "A4 became portrait");
}

#[test]
fn a_custom_paper_edge_is_saved() {
    let sheet = change_save_reopen(|app| {
        panel(app, PanelMsg::SetPaperSize("User:200x150".into()));
        panel(
            app,
            PanelMsg::CustomPaperEdgeTyping(PaperEdge::Width, "250".into()),
        );
        panel(app, PanelMsg::CommitCustomPaperEdge(PaperEdge::Width));
    });
    assert!(sheet.paper_size.contains("250"), "{}", sheet.paper_size);
}

#[test]
fn the_border_title_block_and_zones_are_saved() {
    let sheet = change_save_reopen(|app| {
        for edit in [
            SheetFrameEdit::ShowBorder(false),
            SheetFrameEdit::ShowTitleBlock(false),
            SheetFrameEdit::VerticalZones(2),
            SheetFrameEdit::HorizontalZones(3),
        ] {
            panel(app, PanelMsg::EditSheetFrame(edit));
        }
    });
    let frame = sheet.frame;
    assert!(!frame.show_border && !frame.show_title_block);
    assert_eq!((frame.vertical_zones, frame.horizontal_zones), (2, 3));
}

#[test]
fn the_default_font_is_saved() {
    let sheet = change_save_reopen(|app| {
        panel(app, PanelMsg::SetDocumentFontFamily(Some("Roboto".into())))
    });
    assert_eq!(sheet.font_family.as_deref(), Some("Roboto"));
}

#[test]
fn every_title_block_field_is_saved() {
    for field in [
        TitleBlockField::Title,
        TitleBlockField::DocumentNumber,
        TitleBlockField::Revision,
        TitleBlockField::Date,
        TitleBlockField::Company,
        TitleBlockField::DrawnBy,
        TitleBlockField::CheckedBy,
        TitleBlockField::ApprovedBy,
    ] {
        let sheet = change_save_reopen(|app| {
            panel(app, PanelMsg::TitleBlockTyping(field, "value 7".into()));
            panel(app, PanelMsg::CommitTitleBlockField(field));
        });
        assert_eq!(
            sheet.title_block.get(field.key()).map(String::as_str),
            Some("value 7"),
            "{field:?}"
        );
    }
}

#[test]
fn a_document_options_change_is_one_undo_step() {
    let (mut app, path, _r) = clean_app();
    panel(&mut app, PanelMsg::SetPaperSize("A3".into()));
    panel(
        &mut app,
        PanelMsg::EditSheetFrame(SheetFrameEdit::ShowBorder(false)),
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let doc = app.document_state.engines[&path].document();
    assert_eq!(doc.paper_size, "A4");
    assert!(doc.frame.show_border);
}
