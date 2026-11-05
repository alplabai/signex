//! #2027 - the Sample Project, opened through the real Help / start-screen
//! flow, takes title-block edits: `active_path` is the sheet, the canvas shows
//! the typed value and the real file name, the sheet is dirty, and a Save
//! writes the value.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::Task;
use signex_app::app::{FileMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::{PanelMsg, TitleBlockField};
use signex_app::persist::{DocumentSaves, DocumentSink};
use signex_app::tab_bar::TabMessage;
use signex_output::template::FramePrimitive;

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

/// Run `task` and everything it feeds back, as the iced runtime would.
fn settle(app: &mut Signex, task: Task<Message>) {
    let mut pending = vec![task];
    for _ in 0..40 {
        let Some(task) = pending.pop() else { return };
        for message in run(task) {
            pending.push(app.update(message));
        }
    }
}

fn drawn(app: &Signex) -> Vec<String> {
    app.interaction_state
        .active_canvas()
        .sheet_frame
        .iter()
        .filter_map(|p| match p {
            FramePrimitive::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Both tests unpack into the one per-process sample directory.
static SAMPLE_DIR: Mutex<()> = Mutex::new(());

fn open_sample() -> (Signex, PathBuf, Arc<Recorder>) {
    let (mut app, _t) = Signex::new();
    let root = signex_app::sample_project::sample_root().expect("test config root");
    let _ = std::fs::remove_dir_all(&root);
    let task = app.update(Message::Menu(MenuMessage::OpenSampleProject));
    settle(&mut app, task);
    let sheet = root.join("linear-regulator.snxsch");
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    (app, sheet, recorder)
}

fn type_and_commit(app: &mut Signex, field: TitleBlockField, text: &str) {
    for msg in [
        PanelMsg::TitleBlockTyping(field, text.to_string()),
        PanelMsg::CommitTitleBlockField(field),
    ] {
        let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
    }
}

#[test]
fn the_sample_project_takes_title_block_edits_through_the_real_open_flow() {
    let _dir = SAMPLE_DIR.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, sheet, recorder) = open_sample();

    assert!(
        app.document_state.engines.contains_key(&sheet),
        "sheet open"
    );
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(sheet.as_path()),
        "the opened sheet is the active path"
    );
    assert_eq!(
        app.document_state.tabs[app.document_state.active_tab].path, sheet,
        "and the active tab's path"
    );

    type_and_commit(&mut app, TitleBlockField::DrawnBy, "A. Engineer");
    type_and_commit(&mut app, TitleBlockField::DocumentNumber, "1");

    let texts = drawn(&app);
    for wanted in ["A. Engineer", "1", "linear-regulator.snxsch"] {
        assert!(
            texts.iter().any(|t| t == wanted),
            "{wanted} not in {texts:?}"
        );
    }
    assert!(
        !texts.iter().any(|t| t.starts_with("Untitled")),
        "{texts:?}"
    );
    assert!(app.document_state.is_document_dirty(&sheet), "dirty");

    let task = app.update(Message::File(FileMsg::Save));
    settle(&mut app, task);
    let written = recorder
        .0
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("Save wrote");
    let reopened = signex_types::format::SnxSchematic::parse(&written)
        .expect("reopens")
        .sheet;
    assert_eq!(
        reopened.title_block.get("drawn_by").map(String::as_str),
        Some("A. Engineer")
    );
    assert_eq!(
        reopened
            .title_block
            .get("document_number")
            .map(String::as_str),
        Some("1")
    );
}

/// Opening the sheet again (the project tree's open funnel) and switching
/// tabs keep the active path on the sheet.
#[test]
fn reopening_the_sheet_and_switching_tabs_keep_the_active_path() {
    let _dir = SAMPLE_DIR.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, sheet, _recorder) = open_sample();

    let task = app.update(Message::File(FileMsg::Opened(Some(sheet.clone()))));
    settle(&mut app, task);
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(sheet.as_path())
    );

    let window_id = app
        .ui_state
        .main_window_id
        .unwrap_or_else(iced::window::Id::unique);
    for index in 0..app.document_state.tabs.len() {
        let _ = app.update(Message::Tab {
            window_id,
            msg: TabMessage::Select(index),
        });
        let path = app.document_state.tabs[index].path.clone();
        assert_eq!(
            app.document_state.active_path.as_deref(),
            Some(path.as_path()),
            "tab {index}"
        );
    }
    let sheet_tab = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == sheet)
        .expect("sheet tab");
    let _ = app.update(Message::Tab {
        window_id,
        msg: TabMessage::Select(sheet_tab),
    });
    type_and_commit(&mut app, TitleBlockField::CheckedBy, "B. Reviewer");
    assert!(drawn(&app).iter().any(|t| t == "B. Reviewer"));
    assert!(app.document_state.is_document_dirty(&sheet));
}
