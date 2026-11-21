//! The middle-button drag zoom on the footprint canvas, driven through
//! `canvas::Program::update` with real pointer events.

use iced::event::Event;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, mouse};

use super::{FootprintCanvas, FootprintCanvasState, MAX_SCALE};
use crate::library::editor::footprint::state::{FootprintEditorState, PadsTool};
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};
const PRESS: Point = Point::new(300.0, 200.0);

struct Rig {
    state: FootprintEditorState,
    cache: canvas::Cache,
    address: EditorAddress,
    cstate: FootprintCanvasState,
}

impl Rig {
    fn new() -> Self {
        Self {
            state: FootprintEditorState::empty(),
            cache: canvas::Cache::new(),
            address: EditorAddress::new(
                std::path::PathBuf::from("/tmp/lib.snxlib"),
                "footprints".to_string(),
                signex_library::RowId::new(),
            ),
            cstate: FootprintCanvasState::with_view(20.0, Point::new(400.0, 300.0)),
        }
    }

    fn send(&mut self, event: Event, at: Point) -> Option<FootprintEditorMsg> {
        let fpc = FootprintCanvas {
            state: &self.state,
            address: self.address.clone(),
            bg_color: Color::WHITE,
            grid_color: Color::BLACK,
            cache: &self.cache,
            sketch: None,
            silk_f: &[],
            silk_b: &[],
            zoom_step: 1.1,
            trackpad_pan: false,
            wheel_mode: Default::default(),
        };
        let cursor = mouse::Cursor::Available(at);
        let action = canvas::Program::update(&fpc, &mut self.cstate, &event, BOUNDS, cursor);
        match action?.into_inner().0? {
            LibraryMessage::EditorEvent {
                msg: EditorMsg::Footprint(msg),
                ..
            } => Some(msg),
            other => panic!("unexpected message {other:?}"),
        }
    }

    fn press(&mut self, button: mouse::Button, at: Point) -> Option<FootprintEditorMsg> {
        self.send(Event::Mouse(mouse::Event::ButtonPressed(button)), at)
    }

    fn release(&mut self, button: mouse::Button, at: Point) -> Option<FootprintEditorMsg> {
        self.send(Event::Mouse(mouse::Event::ButtonReleased(button)), at)
    }

    fn move_to(&mut self, at: Point) -> Option<FootprintEditorMsg> {
        let event = Event::Mouse(mouse::Event::CursorMoved { position: at });
        self.send(event, at)
    }
}

#[test]
fn a_middle_drag_up_zooms_the_footprint_in() {
    let mut rig = Rig::new();
    let before = rig.cstate.scale;
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 60.0));
    assert!(rig.cstate.scale > before);
}

#[test]
fn a_middle_drag_down_zooms_the_footprint_out() {
    let mut rig = Rig::new();
    let before = rig.cstate.scale;
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y + 60.0));
    assert!(rig.cstate.scale < before);
}

#[test]
fn a_sideways_middle_drag_leaves_the_footprint_view() {
    let mut rig = Rig::new();
    let (scale, offset) = (rig.cstate.scale, rig.cstate.offset);
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x + 120.0, PRESS.y));
    assert_eq!((rig.cstate.scale, rig.cstate.offset), (scale, offset));
}

#[test]
fn the_footprint_point_under_the_press_stays_put() {
    let mut rig = Rig::new();
    let pinned = rig.cstate.screen_to_world(PRESS);
    rig.press(mouse::Button::Middle, PRESS);
    for (dx, dy) in [(40.0, -30.0), (-70.0, -90.0), (15.0, 25.0), (90.0, -140.0)] {
        rig.move_to(Point::new(PRESS.x + dx, PRESS.y + dy));
        let held = rig.cstate.screen_to_world(PRESS);
        assert!(
            (held.0 - pinned.0).abs() < 1e-2 && (held.1 - pinned.1).abs() < 1e-2,
            "{pinned:?} moved to {held:?}"
        );
    }
}

#[test]
fn a_right_drag_still_pans_the_footprint() {
    let mut rig = Rig::new();
    let (scale, offset) = (rig.cstate.scale, rig.cstate.offset);
    rig.press(mouse::Button::Right, PRESS);
    rig.move_to(Point::new(PRESS.x + 30.0, PRESS.y + 20.0));
    assert_eq!(rig.cstate.scale, scale);
    assert_eq!(
        rig.cstate.offset,
        Point::new(offset.x + 30.0, offset.y + 20.0)
    );
    let released = rig.release(mouse::Button::Right, Point::new(330.0, 220.0));
    assert!(!matches!(
        released,
        Some(FootprintEditorMsg::ShowContextMenu { .. })
    ));
}

#[test]
fn a_right_click_still_opens_the_footprint_menu() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Right, PRESS);
    let released = rig.release(mouse::Button::Right, PRESS);
    assert!(matches!(
        released,
        Some(FootprintEditorMsg::ShowContextMenu { .. })
    ));
}

#[test]
fn a_middle_gesture_leaves_an_armed_footprint_tool_armed() {
    let mut rig = Rig::new();
    rig.state.pads_tool = PadsTool::PlacePad;
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
                FootprintEditorMsg::CursorAt { .. } | FootprintEditorMsg::ViewScaleChanged { .. }
            ),
            "the gesture published {message:?}"
        );
    }
    assert_eq!(rig.state.pads_tool, PadsTool::PlacePad);
    assert!(rig.cstate.drag.is_none(), "the press armed a click");
}

#[test]
fn an_armed_footprint_tools_ghost_follows_the_pointer_through_the_zoom() {
    let mut rig = Rig::new();
    rig.state.pads_tool = PadsTool::PlacePad;
    rig.press(mouse::Button::Middle, PRESS);
    let pointer = Point::new(PRESS.x + 40.0, PRESS.y - 70.0);
    let message = rig.move_to(pointer);
    let Some(FootprintEditorMsg::CursorAt { px_per_mm, .. }) = message else {
        panic!("a zoom tick must report the pointer: {message:?}");
    };
    assert_eq!(px_per_mm, rig.cstate.scale);
    assert!(rig.cstate.last_snap.is_some(), "the snap did not re-run");
}

#[test]
fn the_release_ends_the_footprint_zoom() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.release(mouse::Button::Middle, Point::new(PRESS.x, PRESS.y - 40.0));
    let after = rig.cstate.scale;
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.cstate.scale, after);
}

#[test]
fn the_pointer_leaving_the_window_ends_the_footprint_zoom() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.send(Event::Mouse(mouse::Event::CursorLeft), PRESS);
    assert!(rig.cstate.middle_zoom.is_none());
    let after = rig.cstate.scale;
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.cstate.scale, after);
}

#[test]
fn the_footprint_zoom_stops_at_its_limit() {
    let mut rig = Rig::new();
    rig.press(mouse::Button::Middle, PRESS);
    for step in 1..60 {
        rig.move_to(Point::new(PRESS.x, PRESS.y - 100.0 * step as f32));
    }
    assert_eq!(rig.cstate.scale, MAX_SCALE);
}
