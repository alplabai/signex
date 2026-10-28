//! #1509: the Active Bar right-click hit-test must land on the buttons
//! the view draws, with the draw-mode pill shown and with docked side
//! panels narrowing the canvas.

use super::tests::test_prefs;
use super::*;
use iced::widget::canvas::Program as _;

const WINDOW_WIDTH: f32 = 1400.0;

/// Right-press at `window_x` (canvas docked `canvas_left` px from the
/// window's left edge) and return the published message.
fn right_press_at(
    tool: crate::app::Tool,
    canvas_left: f32,
    canvas_width: f32,
    window_x: f32,
) -> Option<Message> {
    let slot = CanvasSlot::new();
    let overrides = std::collections::HashMap::new();
    let filters = std::collections::HashSet::new();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.current_tool = tool;
    prefs.window_width = WINDOW_WIDTH;
    let view = SchematicCanvas::new(&slot, prefs);
    let bounds = Rectangle {
        x: canvas_left,
        y: 60.0,
        width: canvas_width,
        height: 600.0,
    };
    let at = iced::Point::new(window_x, bounds.y + 20.0);
    let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
    let action = view.update(
        &mut CanvasState::default(),
        &event,
        bounds,
        mouse::Cursor::Available(at),
    )?;
    let (message, _, _) = action.into_inner();
    message
}

fn assert_every_drawn_button_resolves(tool: crate::app::Tool, canvas_left: f32, canvas_width: f32) {
    for (menu, centre) in crate::active_bar::drawn_trigger_centres(tool, WINDOW_WIDTH) {
        let message = right_press_at(tool, canvas_left, canvas_width, centre);
        assert!(
            matches!(
                message,
                Some(Message::ActiveBar(
                    crate::active_bar::ActiveBarMsg::ToggleMenu(m)
                )) if m == menu
            ),
            "{tool:?}: drawn {menu:?} button centre x={centre} resolved to {message:?}"
        );
    }
}

#[test]
fn right_click_resolves_drawn_buttons_while_the_pill_is_shown() {
    assert_every_drawn_button_resolves(crate::app::Tool::Wire, 0.0, WINDOW_WIDTH);
    assert_every_drawn_button_resolves(crate::app::Tool::Bus, 0.0, WINDOW_WIDTH);
}

#[test]
fn right_click_resolves_drawn_buttons_with_docked_side_panels() {
    // Left panel 300 px, right panel 180 px: the canvas centre is no
    // longer the window centre, but the bar still is.
    for tool in [crate::app::Tool::Select, crate::app::Tool::Wire] {
        assert_every_drawn_button_resolves(tool, 300.0, WINDOW_WIDTH - 300.0 - 180.0);
    }
}
