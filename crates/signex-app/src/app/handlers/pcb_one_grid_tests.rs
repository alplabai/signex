//! Caner, 2026-11-21: on a board tab the board's grid is the one grid. The
//! status bar, its popover, the Grid Manager, snap and the grid commands
//! read and write it; other documents keep theirs.

use std::path::PathBuf;

use serde_json::json;
use signex_engine::pcb::placement::grid_step_mm;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::command::CommandArgs;
use crate::app::{EditMsg, Message, Signex, StatusBarRequest, UiMsg};
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
        let _ = self.app.dispatch_command(&command, CommandArgs::none(), None);
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
