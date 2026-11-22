//! Row m161: the Command Log panel.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, Value, json};
use signex_app::app::{Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_app::panels::command_log::{LogOutcome, LogRow, LogSource, ResultFilter, SourceKind};
use signex_app::panels::{CommandLogMsg, PanelMsg};
use signex_app::task_driver::TaskDriver;
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::support::{SheetBench, command, fresh_app, open_sheet, part, redo, undo};
use crate::settle_support::settle;

fn two_parts() -> SheetBench {
    open_sheet(vec![
        part("R1", "10k", (20.0, 20.0)),
        part("R2", "4k7", (100.0, 100.0)),
    ])
}

fn select(b: &mut SheetBench, reference: &str) {
    let id = b.part_id(reference);
    b.app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Symbol)];
}

fn log_action(app: &mut Signex, msg: CommandLogMsg) {
    settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::CommandLog(msg))),
    );
}

fn rows(app: &Signex) -> Vec<LogRow> {
    app.ui_state.command_log.log.rows().cloned().collect()
}

fn shown(app: &Signex) -> Vec<String> {
    app.ui_state
        .command_log
        .log
        .visible()
        .iter()
        .map(|row| row.command.clone())
        .collect()
}

/// A row as the agent socket or the CLI would have logged it.
fn logged(
    command: &str,
    source: LogSource,
    args: Value,
    outcome: LogOutcome,
    document: Option<PathBuf>,
) -> LogRow {
    let operands: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
    LogRow {
        seq: 0,
        at: chrono::Local::now(),
        source,
        command: command.to_string(),
        label: command.to_string(),
        operands,
        outcome,
        took: Duration::from_millis(2),
        document,
        project: None,
        undoable: false,
        view_only: false,
        undone_at: None,
    }
}

fn record(app: &mut Signex, row: LogRow) -> u64 {
    app.ui_state
        .command_log
        .log
        .record(row)
        .expect("the log is not paused")
}

#[test]
fn a_new_session_has_an_empty_log_with_zero_counts() {
    let app = fresh_app();
    let counts = app.ui_state.command_log.log.counts();
    assert_eq!(
        (
            counts.total,
            counts.gui,
            counts.cli,
            counts.agent,
            counts.failed,
            counts.undone
        ),
        (0, 0, 0, 0, 0, 0)
    );
}

#[test]
fn a_command_that_ran_adds_one_row_with_its_id_source_document_and_result() {
    let mut b = two_parts();
    select(&mut b, "R1");
    command(&mut b.app, "delete_selection");
    assert!(!b.has_part("R1"), "the command ran");
    let rows = rows(&b.app);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.command, "delete_selection");
    assert_eq!(row.source, LogSource::Gui { window: None });
    assert_eq!(row.outcome, LogOutcome::Ok(None));
    assert_eq!(row.document.as_ref(), Some(&b.path));
    assert!(row.undoable);
    assert_eq!(
        b.app
            .document_state
            .panel_ctx
            .command_log
            .log
            .rows()
            .count(),
        1,
        "the panel shows the row"
    );
}

#[test]
fn undo_strikes_its_row_through_keeps_it_listed_and_redo_clears_the_mark() {
    let mut b = two_parts();
    select(&mut b, "R1");
    command(&mut b.app, "delete_selection");
    let deleted = rows(&b.app)[0].seq;
    assert!(b.dirty());

    command(&mut b.app, "undo");
    assert!(b.has_part("R1"), "undo restored the part");
    assert!(!b.dirty(), "undo is back at the save point");
    let log = &b.app.ui_state.command_log.log;
    assert!(log.row(deleted).expect("kept").undone_at.is_some());
    assert_eq!(log.counts().undone, 1);

    log_action(
        &mut b.app,
        CommandLogMsg::ResultFilterPicked(ResultFilter::Undone),
    );
    assert_eq!(shown(&b.app), ["delete_selection"]);

    command(&mut b.app, "redo");
    let log = &b.app.ui_state.command_log.log;
    assert!(log.row(deleted).expect("kept").undone_at.is_none());
}

#[test]
fn the_edit_undo_and_redo_messages_leave_the_document_as_the_log_says() {
    let mut b = two_parts();
    select(&mut b, "R1");
    command(&mut b.app, "delete_selection");
    undo(&mut b.app);
    assert!(b.has_part("R1"));
    redo(&mut b.app);
    assert!(!b.has_part("R1"));
}

#[test]
fn a_refused_command_is_a_failed_row_with_its_reason_verbatim() {
    let mut app = fresh_app();
    command(&mut app, "place_wire");
    let row = rows(&app).pop().expect("a row");
    assert_eq!(
        row.outcome,
        LogOutcome::Failed("No schematic is open".to_string())
    );
    assert_eq!(app.ui_state.command_log.log.counts().failed, 1);
    log_action(
        &mut app,
        CommandLogMsg::ResultFilterPicked(ResultFilter::Failed),
    );
    assert_eq!(shown(&app), ["place_wire"]);
}

#[test]
fn a_failed_row_shows_its_code_and_copy_error_copies_the_whole_string() {
    let mut app = fresh_app();
    let reason = "E_LOCKED: U3 is locked (Lock Component)";
    let seq = record(
        &mut app,
        logged(
            "move_component",
            LogSource::Agent {
                client: Some("bringup-script".into()),
                pid: None,
            },
            json!({"designator": "U3"}),
            LogOutcome::Failed(reason.to_string()),
            None,
        ),
    );
    let row = app.ui_state.command_log.log.row(seq).expect("the row");
    assert_eq!(row.outcome.short_text(), Some("E_LOCKED"));
    assert_eq!(row.outcome.text(), Some(reason));

    let mut driver = TaskDriver::new();
    driver
        .send(
            &mut app,
            Message::Dock(DockMessage::Panel(PanelMsg::CommandLog(
                CommandLogMsg::CopyErrorRequested(seq),
            ))),
        )
        .expect("the copy settled");
    assert_eq!(driver.clipboard_text(), Some(reason));
}

#[test]
fn the_source_chips_and_the_search_filter_the_list() {
    let mut app = fresh_app();
    let none = || LogOutcome::Ok(None);
    record(
        &mut app,
        logged(
            "place_wire",
            LogSource::Gui { window: None },
            json!({}),
            none(),
            None,
        ),
    );
    record(
        &mut app,
        logged(
            "check_erc",
            LogSource::of_agent(Some("signex-cli".into()), None),
            json!({}),
            none(),
            None,
        ),
    );
    record(
        &mut app,
        logged(
            "set_param",
            LogSource::of_agent(Some("bringup-script".into()), Some(7)),
            json!({"target": "R5", "value": "31.6k"}),
            none(),
            None,
        ),
    );
    let counts = app.ui_state.command_log.log.counts();
    assert_eq!((counts.gui, counts.cli, counts.agent), (1, 1, 1));

    log_action(&mut app, CommandLogMsg::SourceToggled(SourceKind::Gui));
    assert_eq!(shown(&app), ["check_erc", "set_param"]);
    log_action(&mut app, CommandLogMsg::SourceToggled(SourceKind::Agent));
    assert_eq!(shown(&app), ["check_erc"]);
    log_action(&mut app, CommandLogMsg::SourceToggled(SourceKind::Gui));
    log_action(&mut app, CommandLogMsg::SourceToggled(SourceKind::Agent));

    log_action(&mut app, CommandLogMsg::FilterChanged("31.6K".into()));
    assert_eq!(shown(&app), ["set_param"], "the search matches an argument");
    log_action(
        &mut app,
        CommandLogMsg::FilterChanged("no such text".into()),
    );
    assert!(shown(&app).is_empty());
}

#[test]
fn view_commands_are_logged_but_listed_only_when_switched_on() {
    let mut app = fresh_app();
    command(&mut app, "cycle_selection_mode");
    command(&mut app, "place_wire");
    assert_eq!(rows(&app).len(), 2);
    assert_eq!(shown(&app), ["place_wire"]);
    log_action(&mut app, CommandLogMsg::ViewCommandsToggled);
    assert_eq!(shown(&app), ["cycle_selection_mode", "place_wire"]);
}

#[test]
fn clicking_a_row_selects_it_and_follow_newest_and_more_switch() {
    let mut app = fresh_app();
    command(&mut app, "place_wire");
    let seq = rows(&app)[0].seq;
    log_action(&mut app, CommandLogMsg::RowSelected(seq));
    assert_eq!(app.ui_state.command_log.selected, Some(seq));

    let follow = app.ui_state.command_log.follow_newest;
    log_action(&mut app, CommandLogMsg::FollowNewestToggled);
    assert_eq!(app.ui_state.command_log.follow_newest, !follow);
    log_action(&mut app, CommandLogMsg::MoreToggled);
    assert!(app.ui_state.command_log.more_open);
}

#[test]
fn clear_empties_the_list_and_numbering_goes_on() {
    let mut app = fresh_app();
    command(&mut app, "place_wire");
    let first = rows(&app)[0].seq;
    log_action(&mut app, CommandLogMsg::RowSelected(first));
    log_action(&mut app, CommandLogMsg::Cleared);
    assert!(app.ui_state.command_log.log.is_empty());
    assert_eq!(app.ui_state.command_log.selected, None);
    command(&mut app, "place_wire");
    assert!(rows(&app)[0].seq > first, "a number is never reused");
}

#[test]
fn a_paused_log_records_nothing_until_resumed() {
    let mut app = fresh_app();
    log_action(&mut app, CommandLogMsg::PauseToggled);
    command(&mut app, "place_wire");
    assert!(app.ui_state.command_log.log.is_empty());
    log_action(&mut app, CommandLogMsg::PauseToggled);
    command(&mut app, "place_wire");
    assert_eq!(rows(&app).len(), 1);
}

#[test]
fn replay_runs_the_command_again_as_a_new_gui_row() {
    let mut b = two_parts();
    select(&mut b, "R1");
    command(&mut b.app, "delete_selection");
    let first = rows(&b.app)[0].seq;
    undo(&mut b.app);
    select(&mut b, "R1");
    log_action(&mut b.app, CommandLogMsg::ReplayRequested(first));
    assert!(!b.has_part("R1"), "the command ran again");
    let last = rows(&b.app).pop().expect("a row");
    assert_eq!(last.command, "delete_selection");
    assert_eq!(last.source, LogSource::Gui { window: None });
    assert!(last.seq > first);
}

/// Decision 2026-11-19 (Command Log): Replay runs the recorded arguments,
/// never the current selection.
#[test]
fn replay_never_acts_on_the_current_selection() {
    let mut b = two_parts();
    select(&mut b, "R1");
    command(&mut b.app, "delete_selection");
    let first = rows(&b.app)[0].seq;
    undo(&mut b.app);
    assert!(b.has_part("R1"));
    select(&mut b, "R2");
    log_action(&mut b.app, CommandLogMsg::ReplayRequested(first));
    assert!(
        b.has_part("R2"),
        "the part selected now was not part of the logged command"
    );
}

#[test]
fn copy_as_cli_puts_the_cli_line_with_its_arguments_on_the_clipboard() {
    let mut app = fresh_app();
    let seq = record(
        &mut app,
        logged(
            "set_param",
            LogSource::of_agent(Some("signex-cli".into()), None),
            json!({"target": "R5"}),
            LogOutcome::Ok(None),
            None,
        ),
    );
    let mut driver = TaskDriver::new();
    driver
        .send(
            &mut app,
            Message::Dock(DockMessage::Panel(PanelMsg::CommandLog(
                CommandLogMsg::CopyCliRequested(seq),
            ))),
        )
        .expect("the copy settled");
    assert_eq!(
        driver.clipboard_text(),
        Some("signex-cli run '<project.snxprj>' set_param --arg target=R5")
    );
}

#[test]
fn jump_to_object_selects_the_part_the_row_names() {
    let mut b = two_parts();
    let id = b.part_id("R2");
    let path = b.path.clone();
    let seq = record(
        &mut b.app,
        logged(
            "set_param",
            LogSource::of_agent(Some("bringup-script".into()), None),
            json!({"designator": "R2"}),
            LogOutcome::Ok(None),
            Some(path),
        ),
    );
    log_action(&mut b.app, CommandLogMsg::JumpRequested(seq));
    assert_eq!(
        b.app.interaction_state.active_canvas().selected,
        vec![SelectedItem::new(id, SelectedKind::Symbol)]
    );
    assert!(!b.dirty(), "a jump edits nothing");
}

#[test]
fn jump_to_a_part_no_longer_on_the_sheet_changes_nothing() {
    let mut b = two_parts();
    select(&mut b, "R1");
    let before = b.app.interaction_state.active_canvas().selected.clone();
    let path = b.path.clone();
    let seq = record(
        &mut b.app,
        logged(
            "set_param",
            LogSource::of_agent(None, None),
            json!({"designator": "U99"}),
            LogOutcome::Ok(None),
            Some(path),
        ),
    );
    log_action(&mut b.app, CommandLogMsg::JumpRequested(seq));
    assert_eq!(b.app.interaction_state.active_canvas().selected, before);
}

#[test]
fn save_log_as_writes_a_json_line_per_row_only_when_a_file_is_picked() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("session.jsonl");
    let mut app = fresh_app();
    let answers = HeadlessAnswers::default();
    app.ui_state.io = IoPorts::headless(answers.clone());
    command(&mut app, "place_wire");

    log_action(&mut app, CommandLogMsg::SaveRequested);
    assert!(!file.exists(), "a cancelled dialog writes nothing");

    answers.replace_all([("path".to_string(), file.display().to_string())]);
    log_action(&mut app, CommandLogMsg::SaveRequested);
    let text = std::fs::read_to_string(&file).expect("the log was written");
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["id"], "place_wire");
    assert_eq!(lines[0]["result"], "failed");
}
