//! #2458 - the Command Log panel: every catalog command the session ran,
//! once per command, with its source, arguments and result. The agent-socket
//! rows are pinned beside the agent channel's own tests
//! (`app::agent::handler::command_log_tests`); these drive the GUI side
//! through the real messages.

use std::path::PathBuf;

use signex_app::app::{Message, Signex, TabInfo, TabKind, UiMsg, WindowMsg};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::command_log::{LogOutcome, LogRow, LogSource, ResultFilter};
use signex_app::panels::panel_groups::{PanelGroup, grouped_panels};
use signex_app::panels::{CommandLogMsg, PanelKind, PanelMsg};
use signex_app::task_driver::TaskDriver;

use crate::menu_support::menu_row;

fn log_message(msg: CommandLogMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::CommandLog(msg)))
}

fn rows(app: &Signex) -> Vec<LogRow> {
    app.ui_state.command_log.log.rows().cloned().collect()
}

fn last(app: &Signex) -> LogRow {
    rows(app).pop().expect("a logged row")
}

fn fresh_app() -> Signex {
    let (app, _task) = Signex::new();
    app
}

#[test]
fn a_menu_command_adds_one_gui_row_with_its_id_and_no_operands() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("cycle_selection_mode"));
    let rows = rows(&app);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.command, "cycle_selection_mode");
    assert_eq!(row.source, LogSource::Gui { window: None });
    assert!(row.operands.is_empty(), "a GUI command carries no operands");
    assert_eq!(row.outcome, LogOutcome::Ok(None));
    assert_eq!(row.seq, 1);
    // The panel's copy follows.
    assert_eq!(
        app.document_view.panel_ctx.command_log.log.rows().count(),
        1
    );
}

#[test]
fn a_disabled_command_adds_a_failed_row_with_the_reason() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("place_wire"));
    let row = last(&app);
    assert_eq!(row.command, "place_wire");
    assert_eq!(
        row.outcome,
        LogOutcome::Failed("No schematic is open".to_string())
    );
    assert!(row.undoable, "the catalog says place_wire is an undo step");
    assert_eq!(app.ui_state.command_log.log.counts().failed, 1);
}

#[test]
fn a_query_refused_for_the_gui_is_logged_failed() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("list_documents"));
    let row = last(&app);
    assert_eq!(row.command, "list_documents");
    match &row.outcome {
        LogOutcome::Failed(reason) => {
            assert!(reason.contains("is for agents and the CLI"), "{reason}")
        }
        other => panic!("the refused query was logged as {other:?}"),
    }
}

/// The main window shows `cl-main.snxsch`; an undocked window shows
/// `cl-window.snxsch`, whose engine holds one wire to undo.
fn app_with_an_undocked_sheet() -> (Signex, iced::window::Id) {
    let mut app = fresh_app();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let main = PathBuf::from("cl-main.snxsch");
    let other = PathBuf::from("cl-window.snxsch");
    for path in [&main, &other] {
        let engine =
            signex_engine::Engine::new_with_path(Default::default(), Some(path.to_path_buf()))
                .expect("engine");
        app.document_state
            .engines
            .insert(path.to_path_buf(), engine);
        app.document_state.tabs.push(TabInfo {
            title: path.display().to_string(),
            path: path.to_path_buf(),
            project_id: None,
            kind: TabKind::Schematic,
        });
    }
    app.document_state
        .engines
        .get_mut(&other)
        .expect("the window's engine")
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: signex_types::schematic::Wire {
                uuid: uuid::Uuid::from_u128(2458),
                start: signex_types::schematic::Point::new(10.16, 10.16),
                end: signex_types::schematic::Point::new(20.32, 10.16),
                ..Default::default()
            },
        })
        .expect("a wire");
    let _ = app.update(Message::Window(WindowMsg::UndockTab(1)));
    let window = *app.ui_state.windows.keys().next().expect("undocked window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: other,
        id: window,
    }));
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(main);
    (app, window)
}

#[test]
fn a_command_from_an_undocked_window_adds_exactly_one_row() {
    let (mut app, window) = app_with_an_undocked_sheet();
    let before = rows(&app).len();
    let undo = signex_app::keymap::AppCommandId::new("undo").unwrap();
    let _ = app.update(Message::Ui(UiMsg::MenuInWindow {
        window,
        msg: MenuMessage::Command(undo),
    }));
    let rows = rows(&app);
    assert_eq!(rows.len(), before + 1, "one row for one undo: {rows:?}");
    let row = rows.last().unwrap();
    assert_eq!(row.command, "undo");
    assert_eq!(
        row.source,
        LogSource::Gui {
            window: Some("cl-window.snxsch".to_string())
        }
    );
    assert_eq!(row.document, Some(PathBuf::from("cl-window.snxsch")));
}

#[test]
fn replay_of_an_unlisted_command_is_a_new_refused_gui_row() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("place_wire"));
    let first = last(&app);
    let _ = app.update(log_message(CommandLogMsg::ReplayRequested(first.seq)));
    let rows = rows(&app);
    assert_eq!(rows.len(), 2, "{rows:?}");
    let replayed = &rows[1];
    assert_eq!(replayed.command, "place_wire");
    assert_eq!(replayed.source, LogSource::Gui { window: None });
    assert!(replayed.seq > first.seq);
    // DECISIONS 2026-11-22, "refuses unless proven safe".
    assert!(
        matches!(&replayed.outcome, LogOutcome::Failed(why) if why.contains("can't be replayed")),
        "place_wire is off the safe list: {:?}",
        replayed.outcome
    );
}

#[test]
fn clear_empties_the_log_and_pause_stops_recording() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("place_wire"));
    let _ = app.update(log_message(CommandLogMsg::Cleared));
    assert!(app.ui_state.command_log.log.is_empty());
    assert!(app.document_view.panel_ctx.command_log.log.is_empty());

    let _ = app.update(log_message(CommandLogMsg::PauseToggled));
    let _ = app.update(menu_row("place_wire"));
    assert!(app.ui_state.command_log.log.is_empty(), "paused");

    let _ = app.update(log_message(CommandLogMsg::PauseToggled));
    let _ = app.update(menu_row("place_wire"));
    assert_eq!(rows(&app).len(), 1, "logging again");
}

#[test]
fn view_commands_are_recorded_but_hidden_until_switched_on() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("cycle_selection_mode"));
    let _ = app.update(menu_row("place_wire"));
    let log = &app.ui_state.command_log.log;
    assert_eq!(log.rows().count(), 2);
    let shown: Vec<&str> = log.visible().iter().map(|r| r.command.as_str()).collect();
    assert_eq!(shown, ["place_wire"]);

    let _ = app.update(log_message(CommandLogMsg::ViewCommandsToggled));
    let log = &app.ui_state.command_log.log;
    let shown: Vec<&str> = log.visible().iter().map(|r| r.command.as_str()).collect();
    assert_eq!(shown, ["cycle_selection_mode", "place_wire"]);
}

#[test]
fn the_failed_switch_and_the_search_filter_the_list() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("cycle_selection_mode"));
    let _ = app.update(menu_row("place_wire"));
    let _ = app.update(log_message(CommandLogMsg::ViewCommandsToggled));
    let _ = app.update(log_message(CommandLogMsg::ResultFilterPicked(
        ResultFilter::Failed,
    )));
    let ids = |app: &Signex| -> Vec<String> {
        app.document_view
            .panel_ctx
            .command_log
            .log
            .visible()
            .iter()
            .map(|r| r.command.clone())
            .collect()
    };
    assert_eq!(ids(&app), ["place_wire"]);
    let _ = app.update(log_message(CommandLogMsg::ResultFilterPicked(
        ResultFilter::All,
    )));
    let _ = app.update(log_message(CommandLogMsg::FilterChanged("CYCLE".into())));
    assert_eq!(ids(&app), ["cycle_selection_mode"]);
}

#[test]
fn save_writes_the_log_only_through_the_file_port() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("session.jsonl");
    let mut app = fresh_app();
    let answers = HeadlessAnswers::default();
    app.ui_state.io = IoPorts::headless(answers.clone());
    let _ = app.update(menu_row("place_wire"));
    let _ = app.update(menu_row("list_documents"));

    let task = app.update(log_message(CommandLogMsg::SaveRequested));
    assert!(!file.exists(), "nothing is written inside update");
    answers.replace_all([("path".to_string(), file.display().to_string())]);
    TaskDriver::new()
        .settle(&mut app, task)
        .expect("the save settled");

    let text = std::fs::read_to_string(&file).expect("the log was written");
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], "place_wire");
    assert_eq!(lines[0]["result"], "failed");
    assert_eq!(lines[1]["id"], "list_documents");
}

#[test]
fn a_cancelled_save_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = fresh_app();
    // A headless dialog with no path answer is refused, as a cancel is.
    app.ui_state.io = IoPorts::headless(HeadlessAnswers::default());
    let _ = app.update(menu_row("place_wire"));
    let task = app.update(log_message(CommandLogMsg::SaveRequested));
    TaskDriver::new()
        .settle(&mut app, task)
        .expect("the save settled");
    let written = std::fs::read_dir(dir.path()).expect("read dir").count();
    assert_eq!(written, 0);
}

#[test]
fn copy_as_cli_puts_the_cli_line_on_the_clipboard() {
    let mut app = fresh_app();
    let _ = app.update(menu_row("place_wire"));
    let row = last(&app);
    let mut driver = TaskDriver::new();
    driver
        .send(
            &mut app,
            log_message(CommandLogMsg::CopyCliRequested(row.seq)),
        )
        .expect("the copy settled");
    assert_eq!(
        driver.clipboard_text(),
        Some("signex-cli run '<project.snxprj>' place_wire")
    );
    assert_eq!(
        app.ui_state.command_log.copied, None,
        "the Copied flash ran out"
    );
}

#[test]
fn the_view_row_opens_the_panel_in_the_bottom_dock() {
    let mut app = fresh_app();
    app.document_view.dock.close_panel(PanelKind::CommandLog);
    // The layout save task is dropped: it would write the shared config.
    let _ = app.update(Message::Menu(MenuMessage::OpenCommandLogPanel));
    assert!(matches!(
        app.document_view.dock.locate(PanelKind::CommandLog),
        Some(signex_app::dock::PanelSite::Docked(
            PanelPosition::Bottom,
            _
        ))
    ));
}

#[test]
fn the_panel_list_shows_command_log_under_verify() {
    let grouped = grouped_panels(|_| true);
    let verify = grouped
        .iter()
        .find(|(group, _)| *group == PanelGroup::Verify)
        .map(|(_, kinds)| kinds.clone())
        .expect("a Verify group");
    assert!(verify.contains(&PanelKind::CommandLog), "{verify:?}");
    assert!(!PanelKind::CommandLog.needs_schematic());
    assert!(!PanelKind::CommandLog.needs_pcb());
}

/// Mockup row 161: Pause, Save log as…, Clear and More sit in the panel's
/// title bar, not on the filter row; paused, the first one resumes.
#[test]
fn the_panel_actions_sit_in_the_dock_title_bar() {
    use signex_app::panels::title_actions;
    let mut app = fresh_app();
    let actions = title_actions(PanelKind::CommandLog, &app.document_view.panel_ctx);
    let presses: Vec<Option<CommandLogMsg>> = actions
        .into_iter()
        .map(|action| match action.on_press {
            Some(PanelMsg::CommandLog(msg)) => Some(msg),
            _ => None,
        })
        .collect();
    assert_eq!(
        presses,
        [
            Some(CommandLogMsg::PauseToggled),
            Some(CommandLogMsg::SaveRequested),
            Some(CommandLogMsg::Cleared),
            Some(CommandLogMsg::MoreToggled),
        ]
    );

    let _ = app.update(log_message(CommandLogMsg::PauseToggled));
    let ctx = &app.document_view.panel_ctx;
    let first = title_actions(PanelKind::CommandLog, ctx)
        .into_iter()
        .next()
        .expect("Resume logging");
    assert_eq!(
        first.tip.into_string(),
        signex_i18n::t!(ctx.i18n, signex_i18n::ids::COMMAND_LOG_RESUME).into_string()
    );
}

#[test]
fn the_dock_layout_round_trips_the_command_log_key() {
    let json = serde_json::json!({
        "dock": { "bottom": { "panels": ["command_log"], "collapsed": false } }
    });
    let dock = signex_app::fonts::read_dock_layout_from(Some(&json)).expect("the layout loads");
    assert_eq!(
        dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::CommandLog]
    );
}
