//! Caner, 2026-11-21: on a board tab the board's grid is the one grid. The
//! status bar, its popover, the Grid Manager, snap and the grid commands
//! read and write it; other documents keep theirs.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::json;
use signex_engine::pcb::placement::grid_step_mm;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::command::CommandArgs;
use crate::app::{EditMsg, Message, Signex, StatusBarRequest, UiMsg, WindowMsg};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::{PanelMsg, PcbBoardMsg};
use crate::pcb_canvas::snap_point;
use crate::status_bar::{BoardCounter, BoardPopoverEvent, BoardPopoverModel};
use crate::tab_bar::TabMessage;

const APP_GRID: f32 = 2.54;

fn board(grid_mm: f64) -> PcbBoard {
    serde_json::from_value(json!({
        "uuid": Uuid::now_v7(),
        "setup": {"grid_size": grid_mm}
    }))
    .unwrap()
}

struct Rig {
    app: Signex,
    main: iced::window::Id,
    a: PathBuf,
}

/// A schematic in tab 0, board A (0.5 mm) in tab 1, board B (1 mm) in tab 2,
/// with A in view and an app-wide grid of 2.54 mm.
fn rig() -> Rig {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.unit = signex_types::coord::Unit::Mm;
    app.ui_state.grid_size_mm = APP_GRID;
    app.ui_state.visible_grid_mm = APP_GRID;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: Uuid::now_v7(),
        ..Default::default()
    };
    app.open_schematic_tab(PathBuf::from("/w/s.snxsch"), "s".into(), sheet);
    let a = PathBuf::from("/w/a.snxpcb");
    app.open_pcb_tab(a.clone(), "a".into(), board(0.5));
    app.open_pcb_tab(PathBuf::from("/w/b.snxpcb"), "b".into(), board(1.0));
    let mut rig = Rig { app, main, a };
    rig.select(1);
    rig
}

impl Rig {
    fn select(&mut self, index: usize) {
        let _ = self.app.update(Message::Tab {
            window_id: self.main,
            msg: TabMessage::Select(index),
        });
    }

    fn panel(&mut self, msg: PcbBoardMsg) {
        let msg = Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg)));
        let _ = self.app.update(msg);
    }

    fn popover(&mut self, event: BoardPopoverEvent) {
        let request = StatusBarRequest::BoardPopover(event);
        let _ = self.app.update(Message::Ui(UiMsg::StatusBar(request)));
    }

    fn open_popover(&mut self) {
        let request = StatusBarRequest::BoardCounterClicked {
            window: None,
            counter: BoardCounter::Grid,
        };
        let message = Signex::status_bar_message(self.main, None, request);
        let _ = self.app.update(message);
    }

    fn run(&mut self, id: &str) {
        let command = AppCommandId::new(id).expect("a catalog id");
        let _ = self
            .app
            .dispatch_command(&command, CommandArgs::none(), None);
    }

    fn status(&self) -> f32 {
        self.app.status_grid(self.main).size_mm
    }

    fn board_grid(&self) -> f64 {
        let board = self.app.document_state.pcb_board(&self.a).expect("board");
        grid_step_mm(board)
    }

    /// Where a tool point at (10.03, 10.03) snaps on the board in view.
    fn snapped(&self) -> Point {
        let board = self.app.active_pcb().expect("a board");
        snap_point(board, Point::new(10.03, 10.03), self.app.pcb_snap_view())
    }

    fn assert_one_grid(&self, mm: f64) {
        assert_eq!(self.board_grid(), mm, "the board");
        assert_eq!(self.status(), mm as f32, "the status bar");
        let at = self.snapped();
        let on = ((at.x / mm).round() * mm - at.x).abs() < 1e-6;
        assert!(on, "snap steps by {mm}: {at:?}");
        assert_eq!(
            self.app.ui_state.grid_size_mm, APP_GRID,
            "the app-wide grid stays"
        );
    }
}

#[test]
fn the_grid_manager_shows_in_the_status_bar_and_undoes_in_one_step() {
    let mut rig = rig();
    assert_eq!(rig.status(), 0.5, "the board's own grid, not 2.54");
    rig.panel(PcbBoardMsg::GridStepTyped("0.05".into()));
    rig.panel(PcbBoardMsg::GridStepSubmitted);
    rig.assert_one_grid(0.05);
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    rig.assert_one_grid(0.5);
}

#[test]
fn a_popover_pick_reaches_the_board_and_ticks_its_row() {
    let mut rig = rig();
    rig.open_popover();
    rig.popover(BoardPopoverEvent::GridPresetPicked(1.27));
    rig.assert_one_grid(1.27);
    let model = rig.app.board_popover_model(rig.main).expect("open");
    let BoardPopoverModel::Grid(choice) = model else {
        panic!("not the grid popover");
    };
    assert_eq!(choice.current_mm, 1.27);
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    rig.assert_one_grid(0.5);
}

#[test]
fn a_float_preset_lands_as_its_exact_step() {
    let mut rig = rig();
    rig.open_popover();
    rig.popover(BoardPopoverEvent::GridPresetPicked(0.1));
    assert_eq!(rig.board_grid(), 0.1, "no f32 noise in the document");
}

#[test]
fn the_grid_commands_step_the_boards_grid() {
    let mut rig = rig();
    rig.open_popover();
    rig.popover(BoardPopoverEvent::GridPresetPicked(1.27));
    rig.run("next_grid");
    rig.assert_one_grid(2.54);
    rig.run("previous_grid");
    rig.assert_one_grid(1.27);

    let fast = rig.app.ui_state.fast_grids_mm;
    rig.run("fast_grid_2");
    assert_eq!(rig.status(), fast[1]);
    rig.run("cycle_fast_grid");
    assert_eq!(rig.status(), fast[0], "the cycle reads the board's grid");
    assert_eq!(rig.app.ui_state.grid_size_mm, APP_GRID);
}

#[test]
fn the_fast_grid_chip_and_the_grid_manager_agree() {
    let mut rig = rig();
    rig.panel(PcbBoardMsg::FastGridApplied(0));
    let fast = rig.app.ui_state.fast_grids_mm[0];
    assert_eq!(rig.status(), fast);
}

#[test]
fn each_tab_keeps_its_own_grid() {
    let mut rig = rig();
    rig.panel(PcbBoardMsg::GridStepTyped("0.05".into()));
    rig.panel(PcbBoardMsg::GridStepSubmitted);
    rig.select(2);
    assert_eq!(rig.status(), 1.0, "board B");
    rig.select(0);
    assert_eq!(rig.status(), APP_GRID, "the sheet");
    rig.select(1);
    assert_eq!(rig.status(), 0.05, "board A");
}

/// A step the board already has is no edit, so no undo step.
#[test]
fn picking_the_current_grid_changes_nothing() {
    let mut rig = rig();
    rig.open_popover();
    rig.popover(BoardPopoverEvent::GridPresetPicked(1.27));
    rig.popover(BoardPopoverEvent::GridPresetPicked(1.27));
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    rig.assert_one_grid(0.5);
}

/// A board with no setup takes a pick, keeps it through the file, and one
/// undo gives back no setup.
#[test]
fn a_board_without_a_setup_takes_a_pick_and_undo_gives_none() {
    let mut rig = rig();
    let path = PathBuf::from("/w/bare.snxpcb");
    let bare: PcbBoard = serde_json::from_value(json!({"uuid": Uuid::now_v7()})).unwrap();
    rig.app.open_pcb_tab(path.clone(), "bare".into(), bare);
    rig.select(3);
    rig.open_popover();
    rig.popover(BoardPopoverEvent::GridPresetPicked(0.25));
    assert_eq!(rig.status(), 0.25);
    let board = rig.app.document_state.pcb_board(&path).expect("board");
    let text = signex_types::format::SnxPcb::new(board.clone())
        .write_string()
        .expect("written");
    let read = signex_types::format::SnxPcb::parse(&text).expect("read");
    assert_eq!(grid_step_mm(&read.board), 0.25, "kept by the file");
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    let board = rig.app.document_state.pcb_board(&path).expect("board");
    assert!(board.setup.is_none(), "undo gives back no setup");
}

/// Board B in its own window, board A back in the main one.
fn undock_b(rig: &mut Rig) -> (PathBuf, iced::window::Id) {
    let b = PathBuf::from("/w/b.snxpcb");
    let index = rig
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == b)
        .expect("tab B");
    let before: HashSet<iced::window::Id> = rig.app.ui_state.windows.keys().copied().collect();
    let _ = rig.app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *rig
        .app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("an undocked window");
    let _ = rig
        .app
        .update(Message::Window(WindowMsg::UndockedTabOpened {
            path: b.clone(),
            id: window,
        }));
    let a_tab = rig
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == rig.a)
        .expect("tab A");
    rig.select(a_tab);
    (b, window)
}

/// Every board edit of an undocked window's panel lands on its board: A
/// keeps its revision, so it took no edit and no undo step.
#[test]
fn an_undocked_panel_edits_only_its_own_board() {
    use signex_types::pcb_options::GuideKind;
    let mut rig = rig();
    let (b, window) = undock_b(&mut rig);
    let revision = |app: &Signex, path: &PathBuf| {
        app.document_state
            .board_engine(path)
            .expect("the board")
            .revision()
    };
    let main_before = revision(&rig.app, &rig.a);
    let send = |rig: &mut Rig, msg: PcbBoardMsg| {
        let dock = Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg)));
        let scoped = rig.app.scope_dock_message(window, dock);
        let _ = rig.app.update(scoped);
    };
    let options = |app: &Signex, path: &PathBuf| {
        app.document_state
            .pcb_board(path)
            .expect("the board")
            .options
            .clone()
    };

    send(&mut rig, PcbBoardMsg::GridAdded);
    let grid = options(&rig.app, &b).grids.first().expect("a grid").uuid;
    send(&mut rig, PcbBoardMsg::GridColorStepped(grid));
    send(&mut rig, PcbBoardMsg::GridDeleted(grid));
    assert!(options(&rig.app, &b).grids.is_empty());
    send(&mut rig, PcbBoardMsg::GuideAdded(GuideKind::Vertical));
    let guide = options(&rig.app, &b).guides.first().expect("a guide").uuid;
    send(&mut rig, PcbBoardMsg::GuideEnabledToggled(guide));
    assert!(!options(&rig.app, &b).guides[0].enabled);

    assert!(options(&rig.app, &rig.a).grids.is_empty());
    assert!(options(&rig.app, &rig.a).guides.is_empty());
    assert_eq!(revision(&rig.app, &rig.a), main_before, "A took no edit");
}

/// No wildcard hides a message from the window scope: the session's
/// settings stay app-wide, a document edit does not.
#[test]
fn only_the_board_edits_of_the_panel_are_scoped() {
    assert!(PcbBoardMsg::GridAdded.acts_on_board());
    assert!(PcbBoardMsg::PanelRemoveClicked.acts_on_board());
    assert!(!PcbBoardMsg::SnapToGridsToggled.acts_on_board());
    assert!(!PcbBoardMsg::GuideAddMenuToggled.acts_on_board());
}

/// Caner, 2026-11-21: with an extra grid in effect the status bar names it,
/// and a pick edits that grid as one undo step on that board.
#[test]
fn the_status_bar_shows_and_edits_the_extra_grid_in_effect() {
    let mut rig = rig();
    let mut options = rig
        .app
        .document_state
        .pcb_board(&rig.a)
        .expect("board")
        .options
        .clone();
    options.grids.push(
        serde_json::from_value(json!({
            "uuid": Uuid::from_u128(0xC0), "name": "Parts", "step_mm": 10.0,
            "origin": {"x": 0.0, "y": 0.0}, "color": {"r": 0, "g": 0, "b": 0, "a": 255},
            "components": true, "non_components": false
        }))
        .unwrap(),
    );
    let a = rig.a.clone();
    let command = signex_engine::pcb::BoardCommand::SetBoardOptions {
        options: Box::new(options),
    };
    rig.app
        .execute_board_commands(&a, vec![command], None)
        .expect("the grid");
    let status = rig.app.status_grid(rig.main);
    assert!(status.extra, "idle, the Components grid applies");
    assert_eq!(status.size_mm, 10.0);
    let text = crate::status_bar::board_grid_value(
        &signex_i18n::I18n::en_us(),
        true,
        status.size_mm,
        signex_types::coord::Unit::Mm,
        status.extra,
    )
    .to_string();
    assert!(text.contains("10") && text.contains("(extra)"), "{text}");

    let extra = |rig: &Rig| {
        let board = rig.app.document_state.pcb_board(&rig.a).expect("board");
        board.options.grids[0].step_mm
    };
    rig.open_popover();
    rig.popover(BoardPopoverEvent::GridPresetPicked(1.27));
    assert_eq!(extra(&rig), 1.27, "the extra grid takes the pick");
    assert_eq!(rig.board_grid(), 0.5, "the board's own step stays");
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(extra(&rig), 10.0, "one undo step");

    // A tool snaps to the Non-Components grid, which no extra grid serves.
    rig.run("place_board_via");
    let status = rig.app.status_grid(rig.main);
    assert!(!status.extra);
    assert_eq!(status.size_mm, 0.5);
}

/// A pick or a grid command in an undocked board's window changes that
/// board only: the main window's board, its undo and a sheet's grid stay.
#[test]
fn an_undocked_window_edits_its_own_boards_grid() {
    let mut rig = rig();
    let (b, window) = undock_b(&mut rig);
    let of = |app: &Signex, path: &PathBuf| {
        grid_step_mm(app.document_state.pcb_board(path).expect("board"))
    };

    let request = StatusBarRequest::BoardCounterClicked {
        window: None,
        counter: BoardCounter::Grid,
    };
    let open = Signex::status_bar_message(window, None, request);
    let _ = rig.app.update(open);
    rig.popover(BoardPopoverEvent::GridPresetPicked(0.25));
    assert_eq!(of(&rig.app, &b), 0.25, "the window's board");
    assert_eq!(of(&rig.app, &rig.a), 0.5, "the main board");
    assert_eq!(rig.app.status_grid(window).size_mm, 0.25);
    assert_eq!(rig.status(), 0.5);
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(of(&rig.app, &b), 0.25, "no undo step on the main board");

    let command = AppCommandId::new("fast_grid_2").expect("a catalog id");
    let _ = rig
        .app
        .dispatch_command(&command, CommandArgs::none(), Some(window));
    let fast = rig.app.ui_state.fast_grids_mm[1];
    assert_eq!(rig.app.status_grid(window).size_mm, fast);
    assert_eq!(of(&rig.app, &rig.a), 0.5);
    assert_eq!(rig.app.ui_state.grid_size_mm, APP_GRID, "the sheet's grid");
}

/// A pitch below the nanometre a board stores is refused, not read as the
/// default step.
#[test]
fn a_custom_pitch_too_small_to_store_is_refused() {
    let mut rig = rig();
    rig.open_popover();
    rig.popover(BoardPopoverEvent::CustomGridEdited("0.0000001".into()));
    rig.popover(BoardPopoverEvent::CustomGridSubmitted);
    assert_eq!(rig.board_grid(), 0.5);
    let popover = rig.app.ui_state.board_status.popover.as_ref().unwrap();
    assert!(popover.custom_refused);
}

/// On a sheet the grid commands stay app-wide.
#[test]
fn a_sheet_still_steps_the_app_wide_grid() {
    let mut rig = rig();
    rig.select(0);
    rig.run("fast_grid_1");
    let fast = rig.app.ui_state.fast_grids_mm[0];
    assert_eq!(rig.app.ui_state.grid_size_mm, fast);
    assert_eq!(rig.board_grid(), 0.5, "the board keeps its grid");
}
