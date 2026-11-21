//! The middle-button drag zoom on the board canvas, driven through
//! `canvas::Program::update` with real pointer events.

use iced::{Event, Point, Rectangle, mouse};

use super::camera::MAX_BOARD_SCALE;
use super::*;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};
const PRESS: Point = Point::new(300.0, 200.0);

struct Rig {
    canvas: PcbCanvas,
    state: PcbCanvasState,
}

impl Rig {
    fn new() -> Self {
        Self {
            canvas: PcbCanvas::new(),
            state: PcbCanvasState::default(),
        }
    }

    fn send(&mut self, event: Event, at: Point) -> Option<Message> {
        let cursor = mouse::Cursor::Available(at);
        let action = canvas::Program::update(&self.canvas, &mut self.state, &event, BOUNDS, cursor);
        action.and_then(|action| action.into_inner().0)
    }

    fn press(&mut self, button: mouse::Button, at: Point) -> Option<Message> {
        self.send(Event::Mouse(mouse::Event::ButtonPressed(button)), at)
    }

    fn release(&mut self, button: mouse::Button, at: Point) -> Option<Message> {
        self.send(Event::Mouse(mouse::Event::ButtonReleased(button)), at)
    }

    fn move_to(&mut self, at: Point) -> Option<Message> {
        let event = Event::Mouse(mouse::Event::CursorMoved { position: at });
        self.send(event, at)
    }

    fn scale(&self) -> f32 {
        self.canvas.camera.borrow().scale()
    }

    fn offset(&self) -> Point {
        self.canvas.camera.borrow().offset()
    }

    fn world(&self, at: Point) -> signex_types::pcb::Point {
        self.canvas.screen_to_world(at, BOUNDS)
    }
}

#[test]
fn a_middle_drag_up_zooms_the_board_in() {
    let mut rig = Rig::new();
    let before = rig.scale();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 60.0));
    assert!(rig.scale() > before);
}

#[test]
fn a_middle_drag_down_zooms_the_board_out() {
    let mut rig = Rig::new();
    let before = rig.scale();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y + 60.0));
    assert!(rig.scale() < before);
}

#[test]
fn a_sideways_middle_drag_leaves_the_board_view() {
    let mut rig = Rig::new();
    let (scale, offset) = (rig.scale(), rig.offset());
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x + 120.0, PRESS.y));
    assert_eq!((rig.scale(), rig.offset()), (scale, offset));
}

#[test]
fn the_board_point_under_the_press_stays_put() {
    let mut rig = Rig::new();
    let pinned = rig.world(PRESS);
    rig.press(mouse::Button::Middle, PRESS);
    for (dx, dy) in [(40.0, -30.0), (-70.0, -90.0), (15.0, 25.0), (90.0, -140.0)] {
        rig.move_to(Point::new(PRESS.x + dx, PRESS.y + dy));
        let held = rig.world(PRESS);
        assert!(
            (held.x - pinned.x).abs() < 1e-2 && (held.y - pinned.y).abs() < 1e-2,
            "{pinned:?} moved to {held:?}"
        );
    }
}

#[test]
fn a_right_drag_still_pans_the_board() {
    let mut rig = Rig::new();
    let (scale, offset) = (rig.scale(), rig.offset());
    rig.press(mouse::Button::Right, PRESS);
    rig.move_to(Point::new(PRESS.x + 30.0, PRESS.y + 20.0));
    assert_eq!(rig.scale(), scale);
    assert_eq!(rig.offset(), Point::new(offset.x + 30.0, offset.y + 20.0));
    let released = rig.release(mouse::Button::Right, Point::new(330.0, 220.0));
    assert!(!matches!(released, Some(Message::ContextMenu(_))));
}

#[test]
fn a_right_click_still_opens_the_board_menu() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Right, PRESS);
    let released = rig.release(mouse::Button::Right, PRESS);
    assert!(matches!(
        released,
        Some(Message::ContextMenu(ContextMenuMsg::Show(..)))
    ));
}

#[test]
fn a_middle_gesture_leaves_an_armed_board_tool_armed() {
    let mut rig = Rig::new();
    rig.canvas.set_clicks_only(true);
    let mut messages = vec![rig.press(mouse::Button::Middle, PRESS)];
    for dy in [-40.0, -80.0, -50.0] {
        messages.push(rig.move_to(Point::new(PRESS.x + 10.0, PRESS.y + dy)));
    }
    let end = Point::new(PRESS.x + 10.0, PRESS.y - 50.0);
    messages.push(rig.release(mouse::Button::Middle, end));
    for message in messages.into_iter().flatten() {
        assert!(
            matches!(
                message,
                Message::CanvasEvent(CanvasEvent::ViewMoved)
                    | Message::PcbCanvas(PcbCanvasMsg::PointerMoved { .. })
            ),
            "the gesture published {message:?}"
        );
    }
    assert!(rig.canvas.is_clicks_only());

    rig.press(mouse::Button::Left, end);
    let click = rig.release(mouse::Button::Left, end);
    assert!(
        matches!(click, Some(Message::PcbCanvas(PcbCanvasMsg::Clicked { .. }))),
        "the tool's next click: {click:?}"
    );
}

#[test]
fn an_armed_tools_ghost_follows_the_pointer_through_the_zoom() {
    let mut rig = Rig::new();
    rig.canvas.set_clicks_only(true);
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 20.0));
    let pointer = Point::new(PRESS.x + 40.0, PRESS.y - 70.0);
    let message = rig.move_to(pointer);
    let Some(Message::PcbCanvas(PcbCanvasMsg::PointerMoved { at, .. })) = message else {
        panic!("a zoom tick must report the pointer: {message:?}");
    };
    assert_eq!(at, rig.world(pointer));
}

#[test]
fn the_release_ends_the_board_zoom() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.release(mouse::Button::Middle, Point::new(PRESS.x, PRESS.y - 40.0));
    let after = rig.scale();
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.scale(), after);
}

#[test]
fn the_pointer_leaving_the_window_ends_the_board_zoom() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.send(Event::Mouse(mouse::Event::CursorLeft), PRESS);
    assert!(rig.state.middle_zoom.is_none());
    let after = rig.scale();
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.scale(), after);
}

#[test]
fn the_board_zoom_stops_at_400_px_per_mm() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Middle, PRESS);
    for step in 1..80 {
        rig.move_to(Point::new(PRESS.x, PRESS.y - 100.0 * step as f32));
    }
    assert_eq!(rig.scale(), MAX_BOARD_SCALE);
}
