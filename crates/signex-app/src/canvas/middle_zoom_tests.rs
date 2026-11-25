//! The middle-button drag zoom on the schematic canvas, driven through
//! `canvas::Program::update` with real pointer events.

use super::tests::test_prefs;
use super::*;
use crate::app::Tool;
use iced::widget::canvas::Program as _;
use iced::{Point, Size};

const PRESS: Point = Point::new(300.0, 200.0);

fn bounds() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0))
}

struct Rig {
    slot: CanvasSlot,
    state: CanvasState,
    tool: Tool,
}

impl Rig {
    fn new(tool: Tool) -> Self {
        Self {
            slot: CanvasSlot::new(),
            state: CanvasState::default(),
            tool,
        }
    }

    fn send(&mut self, event: Event, at: Point) -> Option<Message> {
        let (overrides, filters) = Default::default();
        let mut prefs = test_prefs(&overrides, &filters);
        prefs.current_tool = self.tool;
        let view = SchematicCanvas::new(&self.slot, prefs);
        let cursor = mouse::Cursor::Available(at);
        let action = view.update(&mut self.state, &event, bounds(), cursor);
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
        self.slot.camera().scale
    }

    fn world(&self, at: Point) -> Point {
        self.slot.camera().screen_to_world(at, bounds())
    }
}

#[test]
fn a_middle_drag_up_zooms_in() {
    let mut rig = Rig::new(Tool::Select);
    let before = rig.scale();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 60.0));
    assert!(rig.scale() > before);
}

#[test]
fn a_middle_drag_down_zooms_out() {
    let mut rig = Rig::new(Tool::Select);
    let before = rig.scale();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y + 60.0));
    assert!(rig.scale() < before);
}

#[test]
fn a_sideways_middle_drag_changes_nothing() {
    let mut rig = Rig::new(Tool::Select);
    let before = rig.slot.camera().clone();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x + 120.0, PRESS.y));
    let after = rig.slot.camera().clone();
    assert_eq!((after.offset, after.scale), (before.offset, before.scale));
}

#[test]
fn the_press_point_stays_put_while_the_pointer_wanders() {
    let mut rig = Rig::new(Tool::Select);
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
fn a_right_drag_still_pans_and_keeps_the_zoom() {
    let mut rig = Rig::new(Tool::Select);
    let before = rig.slot.camera().clone();
    rig.press(mouse::Button::Right, PRESS);
    rig.move_to(Point::new(PRESS.x + 30.0, PRESS.y + 20.0));
    let after = rig.slot.camera().clone();
    assert_eq!(after.scale, before.scale);
    assert_eq!(
        after.offset,
        Point::new(before.offset.x + 30.0, before.offset.y + 20.0)
    );
    let released = rig.release(mouse::Button::Right, Point::new(330.0, 220.0));
    assert!(!matches!(released, Some(Message::ContextMenu(_))));
}

#[test]
fn a_right_click_still_opens_the_menu() {
    let mut rig = Rig::new(Tool::Select);
    rig.press(mouse::Button::Right, PRESS);
    let released = rig.release(mouse::Button::Right, PRESS);
    assert!(matches!(
        released,
        Some(Message::ContextMenu(ContextMenuMsg::Show(..)))
    ));
}

#[test]
fn a_middle_gesture_leaves_an_armed_tool_and_a_wire_in_progress() {
    let mut rig = Rig::new(Tool::Wire);
    rig.slot.drawing_mode = true;
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
                Message::CanvasEvent(CanvasEvent::ViewMoved | CanvasEvent::CursorAt { .. })
            ),
            "the gesture published {message:?}"
        );
    }
    assert_eq!(rig.tool, Tool::Wire);
    assert!(rig.slot.drawing_mode);
}

#[test]
fn a_middle_click_opens_no_menu_and_selects_nothing() {
    let mut rig = Rig::new(Tool::Select);
    assert!(rig.press(mouse::Button::Middle, PRESS).is_none());
    assert!(rig.release(mouse::Button::Middle, PRESS).is_none());
    assert!(rig.state.middle_zoom.is_none());
}

#[test]
fn the_release_reports_the_sheet_point_under_the_pointer() {
    let mut rig = Rig::new(Tool::Wire);
    rig.press(mouse::Button::Middle, PRESS);
    let end = Point::new(PRESS.x + 50.0, PRESS.y - 60.0);
    rig.move_to(end);
    let expected = rig.world(end);
    let Some(Message::CanvasEvent(CanvasEvent::CursorAt { x, y, .. })) =
        rig.release(mouse::Button::Middle, end)
    else {
        panic!("the release must report the pointer");
    };
    assert_eq!((x, y), (expected.x, expected.y));
}

#[test]
fn the_release_ends_the_zoom() {
    let mut rig = Rig::new(Tool::Select);
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.release(mouse::Button::Middle, Point::new(PRESS.x, PRESS.y - 40.0));
    let after = rig.scale();
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.scale(), after);
}

#[test]
fn the_pointer_leaving_the_window_ends_the_zoom() {
    let mut rig = Rig::new(Tool::Select);
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.send(Event::Mouse(mouse::Event::CursorLeft), PRESS);
    assert!(rig.state.middle_zoom.is_none());
    let after = rig.scale();
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.scale(), after);
}

#[test]
fn the_zoom_stops_at_the_sheet_limit() {
    let mut rig = Rig::new(Tool::Select);
    rig.press(mouse::Button::Middle, PRESS);
    for step in 1..60 {
        rig.move_to(Point::new(PRESS.x, PRESS.y - 100.0 * step as f32));
    }
    assert_eq!(rig.scale(), Camera::MAX_SCALE);
}
