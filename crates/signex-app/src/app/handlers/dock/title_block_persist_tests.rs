//! #2027 - what the canvas shows is what a save, a close and a quit use: a
//! title-block field typed without Enter is committed by the persist handlers
//! themselves, on every route that reaches them.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::Task;
use signex_types::schematic::SchematicSheet;

use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::panels::{PanelMsg, TitleBlockField};
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

fn settle(app: &mut Signex, task: Task<Message>) {
    for message in run(task) {
        let _ = app.update(message);
    }
}

fn open(app: &mut Signex, name: &str, sheet: SchematicSheet) -> PathBuf {
    let path = PathBuf::from(format!("{name}.snxsch"));
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: name.to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    path
}

fn app_with_draft() -> (Signex, PathBuf, Arc<Recorder>) {
    let (mut app, _t) = Signex::new();
    let sheet = open(&mut app, "power", SchematicSheet::default());
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    type_drawn_by(&mut app, "X");
    (app, sheet, recorder)
}

fn type_drawn_by(app: &mut Signex, text: &str) {
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::TitleBlockTyping(TitleBlockField::DrawnBy, text.to_string()),
    )));
}

fn written_drawn_by(recorder: &Recorder) -> Option<String> {
    let bytes = recorder.0.lock().unwrap().last().cloned()?;
    signex_types::format::SnxSchematic::parse(&bytes)
        .expect("reopens")
        .sheet
        .title_block
        .get("drawn_by")
        .cloned()
}

/// File > Save is `MenuMessage::Command("save_document")`, which reaches the
/// save handler without a top-level `Save` message.
#[test]
fn the_save_menu_row_writes_a_field_typed_without_enter() {
    let (mut app, _sheet, recorder) = app_with_draft();
    let task = app.update(Message::Menu(MenuMessage::command("save_document")));
    settle(&mut app, task);
    assert_eq!(written_drawn_by(&recorder).as_deref(), Some("X"));
}

/// Ctrl+S, the palette and a context menu go through `dispatch_command`,
/// which dispatches the bridged message directly.
#[test]
fn the_keymap_command_path_writes_it_too() {
    let (mut app, _sheet, recorder) = app_with_draft();
    let id = crate::keymap::AppCommandId::new("save_document").expect("id");
    let task = app.dispatch_command(&id, crate::app::command::CommandArgs::none(), None);
    settle(&mut app, task);
    assert_eq!(written_drawn_by(&recorder).as_deref(), Some("X"));
}

/// Quit counts a sheet whose only change is a pending draft as unsaved.
#[test]
fn the_quit_prompt_counts_a_draft_only_sheet() {
    let (mut app, sheet, _recorder) = app_with_draft();
    let _ = app.handle_app_quit_requested();
    let confirm = app.ui_state.app_quit_confirm.as_ref().expect("a prompt");
    assert!(
        confirm.dirty_paths.contains(&sheet),
        "{:?}",
        confirm.dirty_paths
    );
}

/// Close Project commits the draft first, so the prompt's Save All writes it
/// and closing the sheet's view cannot prune it.
#[test]
fn close_project_commits_the_draft_before_anything_else() {
    let (mut app, sheet, _recorder) = app_with_draft();
    let _ = app.close_active_project();
    let stored = app.document_state.engines[&sheet]
        .document()
        .title_block
        .get("drawn_by")
        .cloned();
    assert_eq!(stored.as_deref(), Some("X"));
    assert!(app.document_state.is_document_dirty(&sheet));
    assert!(app.document_state.panel_ctx.title_block_drafts.is_empty());
}

/// A draft on another sheet goes through the same read-only guard as any
/// edit: a newer-minor sheet refuses it, stays clean and unchanged, and the
/// refusal is logged.
#[test]
fn a_draft_on_a_read_only_sheet_is_refused_when_another_sheet_saves() {
    let (mut app, _t) = Signex::new();
    let mut newer = SchematicSheet::default();
    newer.unknown.newer_minor_source = Some("snxsch/1.7".into());
    let a = open(&mut app, "a", SchematicSheet::default());
    let b = open(&mut app, "b", newer);
    type_drawn_by(&mut app, "X");
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(a);
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder);

    let task = app.update(Message::Menu(MenuMessage::command("save_document")));
    settle(&mut app, task);

    let doc = app.document_state.engines[&b].document();
    assert!(!doc.title_block.contains_key("drawn_by"), "unchanged");
    assert!(!app.document_state.is_document_dirty(&b), "clean");
    let instance = app.document_state.engines[&b].instance_id();
    assert!(
        app.document_state
            .read_only_refusal_noted
            .contains(&instance),
        "the refusal reached Messages"
    );
}

/// A workspace with project `ProjectA` and its sheet open and active, a draft
/// pending on the sheet, and a recorder behind every write.
fn project_with_draft() -> (Signex, PathBuf, crate::app::ProjectId, Arc<Recorder>) {
    use crate::app::handlers::menu::export::tests as fx;
    let mut app = fx::app_workspace("/w/p", &["top.snxsch"]);
    let id = app.document_state.projects[0].id;
    let sheet = PathBuf::from("/w/p/top.snxsch");
    fx::open_with(&mut app.document_state, &sheet, SchematicSheet::default());
    app.document_state.tabs.push(TabInfo {
        title: "top".to_string(),
        path: sheet.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(sheet.clone());
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    type_drawn_by(&mut app, "X");
    (app, sheet, id, recorder)
}

/// Close Project from the project tree, up to the gate.
fn close_project_to_gate(app: &mut Signex, id: crate::app::ProjectId) {
    use crate::app::{ContextMenuMsg, ProjectMsg, ProjectTreeAction};
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![id.to_string()]),
    )));
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id: id,
        result,
    }));
}

/// The project tree's Close Project reads the dirty marks at its gate, so a
/// sheet whose only change is a draft must already be dirty there: it gets
/// the unsaved-changes prompt instead of closing and losing the value.
#[test]
fn tree_close_project_prompts_for_a_draft_only_sheet() {
    let (mut app, sheet, id, _recorder) = project_with_draft();
    close_project_to_gate(&mut app, id);
    let prompt = app.ui_state.project_close_confirm.as_ref();
    assert!(prompt.is_some(), "the draft is an unsaved edit");
    assert!(
        app.document_state.projects.iter().any(|p| p.id == id),
        "still open"
    );
    assert!(app.document_state.is_document_dirty(&sheet));
}

/// Discard drops the draft with the project: nothing is written, no dirty
/// mark outlives the engines it described, and no draft stays behind.
#[test]
fn discard_on_close_drops_the_draft_and_leaves_no_dirty_marks() {
    use crate::app::{ProjectCloseChoice, ProjectMsg};
    let (mut app, sheet, id, recorder) = project_with_draft();
    close_project_to_gate(&mut app, id);
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(app.document_state.projects.is_empty(), "closed");
    assert!(
        recorder.0.lock().unwrap().is_empty(),
        "the file is untouched"
    );
    assert!(!app.document_state.engines.contains_key(&sheet));
    assert!(!app.document_state.is_document_dirty(&sheet));
    assert!(app.document_state.dirty_document_paths().is_empty());
    assert!(app.document_state.panel_ctx.title_block_drafts.is_empty());
}

/// Closing an unrelated tab, by the user or by a discard path, does not
/// commit another sheet's draft or dirty that sheet.
#[test]
fn closing_an_unrelated_tab_leaves_another_sheets_draft_alone() {
    for user_route in [true, false] {
        let (mut app, _t) = Signex::new();
        let a = open(&mut app, "a", SchematicSheet::default());
        type_drawn_by(&mut app, "X");
        let _b = open(&mut app, "b", SchematicSheet::default());
        let index_of_b = app.document_state.tabs.len() - 1;
        if user_route {
            let _ = app.close_tab_parking_dirty(index_of_b);
        } else {
            let _ = app.close_tab_now(index_of_b);
        }
        assert!(
            !app.document_state.engines[&a]
                .document()
                .title_block
                .contains_key("drawn_by")
        );
        assert!(
            !app.document_state.is_document_dirty(&a),
            "user_route={user_route}"
        );
        assert_eq!(app.document_state.panel_ctx.title_block_drafts.len(), 1);
    }
}

/// The user's own close of the sheet commits that sheet's draft (it parks
/// dirty, like any edit); a discard close drops it.
#[test]
fn closing_the_sheet_commits_on_the_users_route_and_drops_on_a_discard() {
    let (mut app, _t) = Signex::new();
    let a = open(&mut app, "a", SchematicSheet::default());
    type_drawn_by(&mut app, "X");
    let _ = app.close_tab_parking_dirty(0);
    assert!(
        app.document_state.engines[&a]
            .document()
            .title_block
            .contains_key("drawn_by")
    );
    assert!(app.document_state.is_document_dirty(&a));

    let (mut app, _t) = Signex::new();
    let a = open(&mut app, "a", SchematicSheet::default());
    type_drawn_by(&mut app, "X");
    let _ = app.close_tab_now(0);
    assert!(app.document_state.panel_ctx.title_block_drafts.is_empty());
    assert!(
        !app.document_state.engines.contains_key(&a),
        "clean, so dropped"
    );
}
