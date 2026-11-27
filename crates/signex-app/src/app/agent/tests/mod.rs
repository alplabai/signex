//! #1924: the agent channel. The fixture is an app with a project of two
//! sheets open, `b` the active tab with the person at work on it, and the
//! listener reported as up; a request is run as `Signex::update` runs it.

mod channel_forms;
mod channel_writers;
mod check_drc;
mod check_erc;
mod check_requirements;
#[cfg(unix)]
mod client;
mod deferred;
#[cfg(unix)]
mod hold;
mod journal;
mod layout_plan;
#[cfg(unix)]
mod live;
mod live_drawing_edit;
mod net_class_membership;
mod opened;
mod opened_deferred;
mod origin;
mod owed_repaint;
mod placed_move;
#[cfg(unix)]
mod presence;
mod query_surfaces;
mod refusals;
mod request_path;
mod round_trip;
mod sheet_entry_sync;
mod update_pcb;
mod window_layout;

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

pub(super) fn sheet_entry(name: &str) -> SheetEntry {
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
pub(in crate::app) fn app_at_work() -> Signex {
    app_in(path("a.snxsch").parent().unwrap())
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
            variant_settings: Default::default(),
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            agent_journal: Default::default(),
            schematic_design: Default::default(),
            fab_outputs: None,
            router_tool_diameter: None,
            bom_alternates: Vec::new(),
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.ui_state.snap_enabled = false;
    // As the window the person works in: it lists its open projects.
    app.agent.lists_projects = true;
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

/// Run `input` as a request from process `pid`. A test that counts log lines
/// uses a pid no other test sends, because the diagnostics ring is shared by
/// every test thread (#2227).
pub(super) fn ask_raw_as(app: &mut Signex, pid: u32, input: Input) -> Value {
    ask_with_task_as(app, pid, input).0
}

/// [`ask_raw`], and the task the request left for the app to run (a save's
/// write).
pub(super) fn ask_with_task(app: &mut Signex, input: Input) -> (Value, iced::Task<Message>) {
    ask_with_task_as(app, 4242, input)
}

/// [`ask_with_task`] from process `pid`.
fn ask_with_task_as(app: &mut Signex, pid: u32, input: Input) -> (Value, iced::Task<Message>) {
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(pid),
        input,
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
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

/// Open the child `power.snxsch` the fixture's sheets place, so the whole
/// project is in memory and a check that reads the project runs at once
/// instead of waiting for its closed sheets (#2549). The person stays on the
/// tab they were on.
pub(super) fn open_the_child(app: &mut Signex) {
    let person = app.document_state.active_path.clone().expect("a sheet");
    let child = path("power.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uid(SHEET_B + 50),
        ..Default::default()
    };
    fixture::add_sheet_with(app, &child, sheet);
    app.document_state.mark_document_clean(&child);
    fixture::activate(app, &person);
}

/// [`call`] for a command that may wait for its reads (#2199): the task it
/// leaves is driven to its end, then the reply is read.
pub(super) fn call_settled(app: &mut Signex, id: &str, args: Value) -> Value {
    let pending = call_pending(app, id, args);
    settle_pending(app, pending)
}

/// A request sent but not driven yet: what the app left to run, and where its
/// reply arrives.
pub(super) struct Pending {
    task: iced::Task<Message>,
    answer: iced::futures::channel::oneshot::Receiver<String>,
}

/// Send `id` as process 4242 and stop before its task runs, so a test can act
/// as the person while the request's reads are in flight.
pub(super) fn call_pending(app: &mut Signex, id: &str, args: Value) -> Pending {
    let (reply, answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4242),
        input: Input::Line(line(app, id, args)),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    Pending { task, answer }
}

/// Drive `pending` to its end and read the reply.
pub(super) fn settle_pending(app: &mut Signex, pending: Pending) -> Value {
    let Pending { task, mut answer } = pending;
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the request settled");
    let line = answer
        .try_recv()
        .expect("the app did not drop the reply")
        .expect("the app answered");
    serde_json::from_str(&line).expect("a reply is JSON")
}

/// [`call`] from process `pid`.
pub(super) fn call_as(app: &mut Signex, pid: u32, id: &str, args: Value) -> Value {
    let text = line(app, id, args);
    ask_raw_as(app, pid, Input::Line(text))
}

pub(super) fn revision(app: &Signex, file: &str) -> u64 {
    app.document_state.engines[&path(file)].revision()
}

pub(super) fn snapshot(app: &Signex) -> String {
    interaction_snapshot(app)
}
