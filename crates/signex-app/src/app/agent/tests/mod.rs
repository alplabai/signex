//! #1924: the agent channel. The fixture is an app with a project of two
//! sheets open, `b` the active tab with the person at work on it, and the
//! listener reported as up; a request is run as `Signex::update` runs it.

#[cfg(unix)]
mod live;
mod opened;
mod refusals;
mod round_trip;

use serde_json::{Value, json};
use signex_types::document_id::DocumentId;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

use super::{AgentCall, AgentMsg, AgentReply, Input};
use crate::app::command::interaction_snapshot_tests::interaction_snapshot;
use crate::app::command::queries::tests::{WIRE_A, path, sheet, uid};
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::app::state::LoadedProject;
use crate::app::{Message, Signex};

pub(super) const SHEET_A: u128 = 100;
pub(super) const SHEET_B: u128 = 101;

fn sheet_entry(name: &str) -> SheetEntry {
    SheetEntry {
        name: name.into(),
        filename: format!("{name}.snxsch"),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// An app with the project `P` (sheets `a` and `b`) open, the person on
/// `b` with a wire selected, the wire tool, a wire in progress and a
/// command-palette query typed; agent connections on and listening.
pub(super) fn app_at_work() -> Signex {
    app_in(&path("a.snxsch").parent().unwrap().to_path_buf())
}

/// [`app_at_work`] with the project's folder `dir`: a test that writes files
/// gives it a folder of its own.
pub(super) fn app_in(dir: &std::path::Path) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    for (name, id) in [("a.snxsch", SHEET_A), ("b.snxsch", SHEET_B)] {
        fixture::add_sheet_with(&mut app, &dir.join(name), sheet(id));
        app.document_state.mark_document_clean(&dir.join(name));
    }
    let id = app.document_state.mint_project_id();
    let dir = dir.to_path_buf();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            document_id: Some(DocumentId::new()),
            description: None,
            name: "P".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("a.snxsch".into()),
            pcb_file: None,
            sheets: vec![sheet_entry("a"), sheet_entry("b")],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.ui_state.snap_enabled = false;
    fixture::activate(&mut app, &dir.join("b.snxsch"));
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.selected = vec![SelectedItem::new(uid(WIRE_A), SelectedKind::Wire)];
    canvas.wire_preview = vec![Point::new(1.0, 2.0)];
    canvas.drawing_mode = true;
    app.interaction_state.current_tool = crate::app::Tool::Wire;
    let _ = app.update(Message::CommandPalette(
        crate::app::CommandPaletteMsg::QueryChanged("wire".into()),
    ));
    app.ui_state.agent_connections_enabled = true;
    app.agent.endpoint = Some("test-endpoint".into());
    app
}

/// A request line carrying the app's own token.
pub(super) fn line(app: &Signex, id: &str, args: Value) -> String {
    let token = app.agent.token().expect("a token").as_str().to_string();
    json!({"token": token, "id": id, "args": args}).to_string()
}

/// Run `input` as a request from process 4242 and read the reply.
pub(super) fn ask_raw(app: &mut Signex, input: Input) -> Value {
    ask_with_task(app, input).0
}

/// [`ask_raw`], and the task the request left for the app to run (a save's
/// write).
pub(super) fn ask_with_task(app: &mut Signex, input: Input) -> (Value, iced::Task<Message>) {
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4242),
        input,
        reply,
    };
    let task = app.update(Message::File(crate::app::FileMsg::Agent(
        AgentMsg::Request(call),
    )));
    let line = answer
        .try_recv()
        .expect("the app did not drop the reply")
        .expect("the app answered inside update");
    (serde_json::from_str(&line).expect("a reply is JSON"), task)
}

pub(super) fn ask(app: &mut Signex, text: String) -> Value {
    ask_raw(app, Input::Line(text))
}

/// Run the command `id` with `args`, carrying the app's own token.
pub(super) fn call(app: &mut Signex, id: &str, args: Value) -> Value {
    let text = line(app, id, args);
    ask(app, text)
}

pub(super) fn revision(app: &Signex, file: &str) -> u64 {
    app.document_state.engines[&path(file)].revision()
}

pub(super) fn snapshot(app: &Signex) -> String {
    interaction_snapshot(app)
}
