//! Board edits end to end (#661 slice 7): commands through the gateway, undo
//! and redo through the `Edit` messages, dirty, save, close and quit, and the
//! derived data that keys on the revision. No editing UI exists yet, so the
//! commands are driven headless.

use std::path::{Path, PathBuf};

use signex_engine::pcb::{BoardCommand, BoardCommandError, BoardItemId};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::document_files::save_async_tests::{begin_save, press_save};
use crate::app::document::{Document, DocumentKey, UndoTarget};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::pcb_edit::{AgentStep, BoardEditError};
use crate::app::{EditMsg, Message, Signex};

const FP_A: u128 = 0xa;
const FP_B: u128 = 0xb;
const SEG: u128 = 0x101;
const VIA: u128 = 0x201;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Two footprints with one pad each on net 1 (so one ratsnest line), a
/// segment and a via.
fn board() -> PcbBoard {
    let footprint = |uuid: u128, x: f64, pad: u128| {
        serde_json::json!({
            "uuid": id(uuid), "reference": "U", "position": {"x": x, "y": 1.0},
            "rotation": 0.0, "layer": "TopCopper",
            "pads": [{
                "uuid": id(pad), "number": "1", "pad_type": "smd", "shape": "rect",
                "position": {"x": 0.1, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
                "drill": null, "layers": ["TopCopper"],
                "net": {"number": 1, "name": "N1"}
            }]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 40.0, "y": 0.0}, {"x": 40.0, "y": 20.0}],
        "footprints": [footprint(FP_A, 5.0, 0xa1), footprint(FP_B, 15.0, 0xb1)],
        "segments": [{"uuid": id(SEG), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 1.0, "y": 0.0}, "layer": "TopCopper"}],
        "vias": [{"uuid": id(VIA), "position": {"x": 3.0, "y": 0.0},
                  "layers": ["TopCopper", "BottomCopper"]}]
    }))
    .unwrap()
}

fn snapshot(board: &PcbBoard) -> String {
    serde_json::to_string(board).unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn live(app: &Signex, path: &Path) -> String {
    snapshot(app.document_state.pcb_board(path).expect("board"))
}

fn dirty(app: &Signex, path: &Path) -> bool {
    let key = DocumentKey::Pcb(path.to_path_buf());
    let by_document = app.document(&key).expect("document").is_dirty();
    let by_set = app.document_state.is_document_dirty(path);
    assert_eq!(by_document, by_set, "the Document query and the set agree");
    by_set
}

fn revision(app: &Signex, path: &Path) -> u64 {
    app.document_state.pcb_revisions[path]
}

fn move_to(footprint: u128, x: f64) -> BoardCommand {
    BoardCommand::MoveFootprint {
        footprint: id(footprint),
        to: Point { x, y: x / 3.0 },
    }
}

fn run(app: &mut Signex, path: &Path, command: BoardCommand) {
    app.execute_board_commands(path, vec![command], None)
        .expect("the command runs");
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn redo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Redo));
}

#[test]
fn commands_dispatched_through_the_gateway_undo_and_redo_exactly() {
    let commands = [
        move_to(FP_A, 0.1 + 0.2),
        BoardCommand::Rotate {
            footprint: id(FP_A),
            degrees: 90.1,
        },
        BoardCommand::Flip {
            footprint: id(FP_B),
        },
        BoardCommand::SetBoardOutline {
            outline: vec![Point { x: 1.1, y: 2.2 }, Point { x: 3.3, y: 4.4 }],
        },
        BoardCommand::DeleteItems {
            items: vec![
                BoardItemId::Segment(id(SEG)),
                BoardItemId::Via(id(VIA)),
                BoardItemId::Footprint(id(FP_A)),
            ],
        },
    ];
    for command in commands {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_board(dir.path());
        let before = live(&app, &path);
        run(&mut app, &path, command.clone());
        let after = live(&app, &path);
        assert_ne!(before, after, "{command:?}");
        undo(&mut app);
        assert_eq!(live(&app, &path), before, "undo of {command:?}");
        redo(&mut app);
        assert_eq!(live(&app, &path), after, "redo of {command:?}");
    }
}

#[test]
fn the_save_point_and_dirty_follow_edit_undo_and_redo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    assert!(!dirty(&app, &path));

    run(&mut app, &path, move_to(FP_A, 7.0));
    assert!(dirty(&app, &path));
    undo(&mut app);
    assert!(!dirty(&app, &path), "undo back to the file reads clean");
    redo(&mut app);
    assert!(dirty(&app, &path), "redo away from the file reads dirty");

    press_save(&mut app);
    assert!(!dirty(&app, &path));
    let saved =
        signex_types::format::SnxPcb::new(app.document_state.pcb_board(&path).unwrap().clone())
            .write_string()
            .unwrap()
            .into_bytes();
    assert_eq!(std::fs::read(&path).unwrap(), saved, "the edit is on disk");

    undo(&mut app);
    assert!(dirty(&app, &path), "undo past the saved board is unsaved");
    redo(&mut app);
    assert!(!dirty(&app, &path), "redo back to the saved board is clean");

    run(&mut app, &path, move_to(FP_B, 9.0));
    undo(&mut app);
    assert!(!dirty(&app, &path));
}

#[test]
fn an_edit_while_the_write_runs_keeps_the_board_dirty_through_any_undo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, &path, move_to(FP_A, 7.0));
    let landed = begin_save(&mut app);
    run(&mut app, &path, move_to(FP_A, 8.0));
    for message in landed {
        let _ = app.update(message);
    }
    assert!(dirty(&app, &path), "the file holds an older snapshot");
    // Undo to the very state the file holds: no history step proves it.
    undo(&mut app);
    assert!(dirty(&app, &path));
    press_save(&mut app);
    assert!(!dirty(&app, &path));
}

#[test]
fn a_refused_command_leaves_the_board_clean_and_the_revision_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let (before, rev) = (live(&app, &path), revision(&app, &path));
    let refused =
        app.execute_board_commands(&path, vec![move_to(FP_A, 3.0), move_to(0xdead, 3.0)], None);
    assert_eq!(
        refused,
        Err(BoardEditError::Refused(
            BoardCommandError::FootprintNotFound(id(0xdead))
        ))
    );
    assert_eq!((live(&app, &path), revision(&app, &path)), (before, rev));
    assert!(!dirty(&app, &path));
    assert!(matches!(
        app.execute_board_commands(Path::new("/nowhere.snxpcb"), vec![move_to(FP_A, 1.0)], None),
        Err(BoardEditError::NotLoaded(_))
    ));
}

#[test]
fn a_read_only_board_cannot_be_edited() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("newer.snxpcb");
    let mut newer = board();
    newer.unknown.newer_minor_source = Some("snxpcb/1.9".into());
    app.open_pcb_tab(path.clone(), "newer".into(), newer);
    let refused = app.execute_board_commands(&path, vec![move_to(FP_A, 1.0)], None);
    assert_eq!(refused, Err(BoardEditError::ReadOnly(path.clone())));
    assert!(!dirty(&app, &path));
}

#[test]
fn the_revision_rises_on_every_edit_undo_and_redo_and_the_ratsnest_follows() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let lines = |app: &Signex| format!("{:?}", app.interaction_state.pcb_ratsnest.lines());
    let start = lines(&app);
    assert_ne!(start, "[]", "two pads on one net give a line");
    let mut seen = vec![revision(&app, &path)];

    run(&mut app, &path, move_to(FP_B, 25.0));
    seen.push(revision(&app, &path));
    let moved = lines(&app);
    assert_ne!(moved, start, "the cache was keyed on the old revision");
    undo(&mut app);
    seen.push(revision(&app, &path));
    assert_eq!(lines(&app), start);
    redo(&mut app);
    seen.push(revision(&app, &path));
    assert_eq!(lines(&app), moved);
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
}

#[test]
fn a_reopened_board_starts_above_every_revision_it_reached() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    for i in 0..5 {
        run(&mut app, &path, move_to(FP_A, 6.0 + f64::from(i)));
    }
    let reached = revision(&app, &path);
    let _ = app.close_tab_now(0);
    app.open_pcb_tab(path.clone(), "board".into(), board());
    assert!(revision(&app, &path) > reached);
}

#[test]
fn a_dirty_board_closed_after_an_engine_edit_is_kept_prompts_at_quit_and_keeps_its_history() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let original = live(&app, &path);
    run(&mut app, &path, move_to(FP_A, 7.0));
    let edited = live(&app, &path);

    let _ = app.close_tab_parking_dirty(0);
    assert!(app.document_state.tabs.is_empty());
    assert!(dirty(&app, &path));
    assert_eq!(live(&app, &path), edited, "the parked board holds the edit");
    let _ = app.handle_app_quit_requested();
    assert!(app.ui_state.app_quit_confirm.is_some(), "quit asks first");

    assert!(app.attach_parked_pcb_tab(path.clone(), "board".into()));
    assert_eq!(live(&app, &path), edited);
    undo(&mut app);
    assert_eq!(live(&app, &path), original, "the history came back");
    assert!(!dirty(&app, &path));
}

#[test]
fn a_discarded_board_forgets_its_edits_and_its_history() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, &path, move_to(FP_A, 7.0));
    let _ = app.close_tab_now(0);
    assert!(!dirty(&app, &path));
    assert!(!app.document_state.holds_pcb(&path));
}

#[test]
fn the_depth_caps_a_boards_history_and_lowering_it_trims_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    assert!(app.set_undo_depth(3));
    for i in 0..5 {
        run(&mut app, &path, move_to(FP_A, 6.0 + f64::from(i)));
    }
    let held = |app: &Signex| app.document_state.board_engine(&path).unwrap().undo_len();
    assert_eq!(held(&app), 3);
    assert!(app.set_undo_depth(2));
    assert_eq!(held(&app), 2);
    // A parked board is trimmed too.
    let _ = app.close_tab_parking_dirty(0);
    assert!(app.set_undo_depth(1));
    assert_eq!(held(&app), 1);
}

#[test]
fn undo_is_offered_when_the_board_has_a_step_and_not_otherwise() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let target = UndoTarget::Document(DocumentKey::Pcb(path.clone()));
    assert_eq!(app.undo_target(None), target);
    assert_eq!(app.undo_availability(&target), (false, false));
    run(&mut app, &path, move_to(FP_A, 7.0));
    assert_eq!(app.undo_availability(&target), (true, false));
    undo(&mut app);
    assert_eq!(app.undo_availability(&target), (false, true));
    // Nothing to undo: the key does nothing and says nothing broke.
    let before = live(&app, &path);
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
}

#[test]
fn an_agent_step_is_labelled_and_a_stale_revision_is_refused_whole() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let read = revision(&app, &path);
    run(&mut app, &path, move_to(FP_A, 7.0));
    let (before, current) = (live(&app, &path), revision(&app, &path));

    let stale = AgentStep {
        label: "Agent: tidy",
        expected_revision: read,
    };
    let refused = app.execute_board_commands(&path, vec![move_to(FP_B, 30.0)], Some(stale));
    assert_eq!(
        refused,
        Err(BoardEditError::Refused(BoardCommandError::StaleRevision {
            expected: read,
            current
        }))
    );
    assert_eq!(
        (live(&app, &path), revision(&app, &path)),
        (before.clone(), current)
    );

    let fresh = AgentStep {
        label: "Agent: tidy",
        expected_revision: current,
    };
    let patches = app
        .execute_board_commands(
            &path,
            vec![move_to(FP_B, 30.0), move_to(FP_A, 8.0)],
            Some(fresh),
        )
        .unwrap();
    assert_eq!(patches.len(), 2);
    let engine = app.document_state.board_engine(&path).unwrap();
    assert_eq!(engine.undo_label(), Some("Agent: tidy"));
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "both edits are one undo step");
}

fn enabled(app: &Signex, command: &str) -> bool {
    let id = crate::keymap::AppCommandId::new(command).unwrap();
    crate::app::command::is_enabled(&id, &app.command_context(None))
}

fn menu_row(app: &mut Signex, command: &str) {
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::command(
        command,
    )));
}

fn dispatch(app: &mut Signex, command: &str) {
    let id = crate::keymap::AppCommandId::new(command).unwrap();
    let _ = app.dispatch_command(&id, crate::app::command::CommandArgs::none(), None);
}

/// Edit > Undo / Redo (the menu row), the toolbar and Active Bar buttons,
/// Ctrl+Z / Ctrl+Y and the palette all run the `undo` / `redo` command; the
/// menu row is a message of its own, the others are `dispatch_command`.
#[test]
fn every_undo_entry_point_steps_the_active_board_and_is_greyed_without_history() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let original = live(&app, &path);
    assert!(!enabled(&app, "undo") && !enabled(&app, "redo"), "greyed");

    let press = |app: &mut Signex, entry: usize, command: &str| match entry {
        0 => menu_row(app, command),
        1 => dispatch(app, command),
        _ => {
            let edit = if command == "undo" {
                EditMsg::Undo
            } else {
                EditMsg::Redo
            };
            let _ = app.update(Message::Edit(edit));
        }
    };
    for (entry, name) in ["menu row", "key, toolbar, palette", "Edit message"]
        .into_iter()
        .enumerate()
    {
        run(&mut app, &path, move_to(FP_A, 7.0));
        let edited = live(&app, &path);
        assert!(enabled(&app, "undo") && !enabled(&app, "redo"), "{name}");
        press(&mut app, entry, "undo");
        assert_eq!(live(&app, &path), original, "{name}: undo");
        assert!(!enabled(&app, "undo") && enabled(&app, "redo"), "{name}");
        press(&mut app, entry, "redo");
        assert_eq!(live(&app, &path), edited, "{name}: redo");
        press(&mut app, entry, "undo");
        assert_eq!(live(&app, &path), original, "{name}: back");
    }
}

/// The title bar count, the tab dot and Save All read the board's dirty
/// state from the engine's history.
#[test]
fn the_title_count_and_tab_dot_follow_the_boards_history() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let dot = |app: &Signex| {
        crate::tab_bar::dirty_tab_paths(app, &app.document_state.tabs).contains(&path)
    };
    assert!((dot(&app), app.document_state.dirty_document_count()) == (false, 0));
    run(&mut app, &path, move_to(FP_A, 7.0));
    assert!((dot(&app), app.document_state.dirty_document_count()) == (true, 1));
    assert!(enabled(&app, "save"));
    menu_row(&mut app, "undo");
    assert!((dot(&app), app.document_state.dirty_document_count()) == (false, 0));
}

use crate::app::command::{CommandArgs, CommandOutcome};

fn undo_by_document(app: &mut Signex, command: &str, document: &str) -> CommandOutcome {
    let id = crate::keymap::AppCommandId::new(command).unwrap();
    let operand = format!("document={document}");
    let args = CommandArgs::parse_operands(&id, &[operand.as_str()]).expect("operands parse");
    app.run_operands(&id, &args).0
}

fn drain(task: iced::Task<Message>) -> Vec<Message> {
    use futures::StreamExt;
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

fn encoded(board: &PcbBoard) -> Vec<u8> {
    signex_types::format::SnxPcb::new(board.clone())
        .write_string()
        .unwrap()
        .into_bytes()
}

#[test]
fn undo_by_document_steps_an_open_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let original = live(&app, &path);
    let document = id(1).to_string();
    assert!(matches!(
        undo_by_document(&mut app, "undo", &document),
        CommandOutcome::Disabled { .. }
    ));
    run(&mut app, &path, move_to(FP_A, 7.0));
    let edited = live(&app, &path);
    assert_eq!(
        undo_by_document(&mut app, "undo", &document),
        CommandOutcome::Ran
    );
    assert_eq!(live(&app, &path), original);
    assert!(!dirty(&app, &path));
    assert_eq!(
        undo_by_document(&mut app, "redo", &document),
        CommandOutcome::Ran
    );
    assert_eq!(live(&app, &path), edited);
    assert!(dirty(&app, &path));
}

/// Review of #2235: a parked board could be stepped while its save was in
/// flight, and the landing save then cleaned the undone state away.
#[test]
fn a_parked_board_cannot_be_stepped_so_a_save_in_flight_loses_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, &path, move_to(FP_A, 7.0));
    let edited_board = app.document_state.pcb_board(&path).unwrap().clone();
    let edited = live(&app, &path);
    let _ = app.close_tab_parking_dirty(0);

    // Save All starts the write of the parked board.
    let task = app
        .begin_pcb_save(path.clone())
        .expect("save starts")
        .expect("a dirty board has bytes to write");
    let landed = drain(task);

    // Every way of stepping or editing the parked board is refused.
    let document = id(1).to_string();
    let CommandOutcome::Disabled { reason } = undo_by_document(&mut app, "undo", &document) else {
        panic!("undo of a parked board must be refused");
    };
    assert!(reason.contains("reopen"), "{reason}");
    assert!(!app.step_board_history(&path, true));
    assert_eq!(
        app.execute_board_commands(&path, vec![move_to(FP_B, 9.0)], None),
        Err(BoardEditError::Parked(path.clone()))
    );
    assert_eq!(
        app.document_state.board_history_available(&path),
        (false, false)
    );
    assert_eq!(live(&app, &path), edited);
    assert!(dirty(&app, &path));
    for parked in app.document_state.parked_pcb_boards.keys() {
        assert!(
            app.document_state.is_document_dirty(parked),
            "a parked board is held only while it is dirty"
        );
    }

    for message in landed {
        let _ = app.update(message);
    }
    assert!(!dirty(&app, &path));
    assert!(!app.document_state.holds_pcb(&path), "released once saved");
    assert_eq!(std::fs::read(&path).unwrap(), encoded(&edited_board));
}

/// A completion for an older revision never cleans a board whose tab is
/// closed either.
#[test]
fn a_stale_completion_never_cleans_a_board_without_a_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, &path, move_to(FP_A, 7.0));
    let landed = begin_save(&mut app);
    run(&mut app, &path, move_to(FP_A, 8.0));
    let _ = app.close_tab_parking_dirty(0);
    for message in landed {
        let _ = app.update(message);
    }
    assert!(dirty(&app, &path), "the file holds an older snapshot");
    assert!(app.document_state.holds_pcb(&path));
}

fn undock(app: &mut Signex, path: &Path) -> iced::window::Id {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "board".into(),
        },
    );
    let canvas = app.new_pcb_canvas_for(path).expect("board canvas");
    app.interaction_state.pcb_canvases.insert(window, canvas);
    window
}

/// The main window shows a schematic tab; the board is undocked.
fn board_undocked_behind_a_schematic(dir: &Path) -> (Signex, PathBuf, iced::window::Id) {
    let (mut app, path) = app_with_board(dir);
    let window = undock(&mut app, &path);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "sheet".into(),
        path: dir.join("sheet.snxsch"),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, path, window)
}

fn drawn(app: &Signex, window: iced::window::Id) -> String {
    let canvas = &app.interaction_state.pcb_canvases[&window];
    format!("{:?}", canvas.active_renderer_snapshot().expect("snapshot"))
}

#[test]
fn undo_in_an_undocked_board_window_steps_that_board_and_repaints_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path, window) = board_undocked_behind_a_schematic(dir.path());
    let original = live(&app, &path);
    let drawn_before = drawn(&app, window);
    run(&mut app, &path, move_to(FP_A, 30.0));
    let drawn_edited = drawn(&app, window);
    assert_ne!(drawn_edited, drawn_before, "the edit repaints the window");

    assert!(
        app.undo_availability(&app.undo_target(Some(window))).0,
        "the window's menu offers Undo"
    );
    let undo_id = crate::keymap::AppCommandId::new("undo").unwrap();
    let _ = app.dispatch_command(&undo_id, CommandArgs::none(), Some(window));
    assert_eq!(live(&app, &path), original);
    assert_eq!(drawn(&app, window), drawn_before);
    assert!(!dirty(&app, &path));

    let redo_id = crate::keymap::AppCommandId::new("redo").unwrap();
    let _ = app.dispatch_command(&redo_id, CommandArgs::none(), Some(window));
    assert_eq!(drawn(&app, window), drawn_edited);
}

#[test]
fn an_agent_edit_to_a_board_that_is_not_the_active_tab_repaints_its_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path, window) = board_undocked_behind_a_schematic(dir.path());
    let before = drawn(&app, window);
    let step = AgentStep {
        label: "Agent: nudge",
        expected_revision: revision(&app, &path),
    };
    app.execute_board_commands(&path, vec![move_to(FP_B, 33.0)], Some(step))
        .unwrap();
    assert_ne!(drawn(&app, window), before);
    let engine = app.document_state.board_engine(&path).unwrap();
    assert_eq!(engine.undo_label(), Some("Agent: nudge"));
}

#[test]
fn a_board_edit_does_not_cancel_the_active_tool() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.interaction_state.current_tool = crate::app::Tool::Wire;
    run(&mut app, &path, move_to(FP_A, 7.0));
    undo(&mut app);
    assert_eq!(app.interaction_state.current_tool, crate::app::Tool::Wire);
}
