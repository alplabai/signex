//! Caner, 2026-11-21: the canvas shows the OS arrow while the selection
//! tool is idle and the crosshair only while a tool is armed, on the
//! schematic and on the board, read through the cursor the view asks for.

use std::path::PathBuf;

use iced::widget::canvas::Program as _;
use iced::{Rectangle, mouse};

use crate::app::{FileMsg, InputMsg, Message, Signex, Tool};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::toolbar::ToolMessage;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 400.0,
    height: 300.0,
};

fn inside() -> mouse::Cursor {
    mouse::Cursor::Available(iced::Point::new(200.0, 150.0))
}

fn app() -> (Signex, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    (app, main)
}

fn escape(app: &mut Signex) {
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
}

fn schematic_cursor(app: &Signex, window: iced::window::Id) -> mouse::Interaction {
    let slot = crate::canvas::CanvasSlot::new();
    let canvas = crate::canvas::SchematicCanvas::new(&slot, app.canvas_view_prefs(window));
    let state = crate::canvas::CanvasState::default();
    canvas.mouse_interaction(&state, BOUNDS, inside())
}

fn board_cursor(app: &Signex) -> mouse::Interaction {
    let state = crate::pcb_canvas::PcbCanvasState::default();
    app.interaction_state
        .pcb_canvas
        .mouse_interaction(&state, BOUNDS, inside())
}

#[test]
fn the_schematic_shows_the_arrow_until_a_tool_is_armed_and_after_esc() {
    let (mut app, main) = app();
    assert_eq!(schematic_cursor(&app, main), mouse::Interaction::Idle);

    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    assert_eq!(schematic_cursor(&app, main), mouse::Interaction::Crosshair);

    escape(&mut app);
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
    assert_eq!(schematic_cursor(&app, main), mouse::Interaction::Idle);

    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Measure)));
    assert_eq!(schematic_cursor(&app, main), mouse::Interaction::Crosshair);
}

#[test]
fn the_board_shows_the_arrow_until_a_tool_is_armed_and_after_esc() {
    let (mut app, _main) = app();
    let path = PathBuf::from("/w/cursor.snxpcb");
    let board: signex_types::pcb::PcbBoard =
        serde_json::from_value(serde_json::json!({"uuid": uuid::Uuid::from_u128(1)})).unwrap();
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path,
        title: "cursor".to_string(),
        result: Ok(Box::new(board)),
    }));
    assert!(app.has_active_pcb());
    assert_eq!(board_cursor(&app), mouse::Interaction::Idle);

    let via = AppCommandId::new("place_board_via").expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(via)));
    assert!(
        app.interaction_state.pcb_tool.is_some(),
        "the Via tool is armed"
    );
    assert_eq!(board_cursor(&app), mouse::Interaction::Crosshair);

    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none(), "Esc ends it");
    assert_eq!(board_cursor(&app), mouse::Interaction::Idle);
}

#[test]
fn the_cursor_rule_is_the_arrow_idle_and_the_crosshair_armed() {
    assert_eq!(super::tool_cursor(false), mouse::Interaction::Idle);
    assert_eq!(super::tool_cursor(true), mouse::Interaction::Crosshair);
}
