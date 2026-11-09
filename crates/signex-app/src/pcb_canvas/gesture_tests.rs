//! The left-button gesture on the PCB canvas, driven through
//! `canvas::Program::update` with synthetic pointer events (#2191).

use iced::keyboard::{Event as KeyEvent, Modifiers};
use iced::{Event, Rectangle, mouse};

use super::*;
use crate::pcb_canvas::PcbCanvasState;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 400.0,
    height: 300.0,
};

struct Rig {
    canvas: PcbCanvas,
    state: PcbCanvasState,
}

fn rig() -> Rig {
    Rig {
        canvas: PcbCanvas::new(),
        state: PcbCanvasState::default(),
    }
}

fn at(x: f32, y: f32) -> mouse::Cursor {
    mouse::Cursor::Available(iced::Point::new(x, y))
}

impl Rig {
    fn send(&mut self, event: Event, cursor: mouse::Cursor) -> Option<PcbCanvasMsg> {
        let action = canvas::Program::update(&self.canvas, &mut self.state, &event, BOUNDS, cursor);
        match action?.into_inner().0? {
            Message::PcbCanvas(msg) => Some(msg),
            Message::ContextMenu(ContextMenuMsg::Close) => None,
            other => panic!("unexpected message {other:?}"),
        }
    }

    fn mods(&mut self, mods: Modifiers) {
        let event = Event::Keyboard(KeyEvent::ModifiersChanged(mods));
        self.send(event, at(0.0, 0.0));
    }

    fn press(&mut self, x: f32, y: f32) {
        let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        self.send(event, at(x, y));
    }

    fn move_to(&mut self, x: f32, y: f32) -> Option<PcbCanvasMsg> {
        let position = iced::Point::new(x, y);
        self.send(
            Event::Mouse(mouse::Event::CursorMoved { position }),
            at(x, y),
        )
    }

    fn release(&mut self, x: f32, y: f32) -> Option<PcbCanvasMsg> {
        let event = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        self.send(event, at(x, y))
    }

    fn world(&self, x: f32, y: f32) -> Point {
        self.canvas.screen_to_world(iced::Point::new(x, y), BOUNDS)
    }
}

#[test]
fn a_press_and_release_in_place_is_a_click_at_the_press() {
    let mut rig = rig();
    rig.press(100.0, 80.0);
    assert_eq!(
        rig.move_to(102.0, 81.0)
            .map(|m| matches!(m, PcbCanvasMsg::PointerMoved { .. })),
        Some(true)
    );
    let msg = rig.release(102.0, 81.0).expect("a click");
    let PcbCanvasMsg::Clicked { at, mode, .. } = msg else {
        panic!("not a click: {msg:?}");
    };
    assert_eq!(at, rig.world(100.0, 80.0));
    assert_eq!(mode, ClickMode::Replace);
}

#[test]
fn a_drag_past_the_threshold_is_a_box_from_the_press() {
    let mut rig = rig();
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    assert!(rig.canvas.marquee.get().is_some());
    let msg = rig.release(160.0, 140.0).expect("a box");
    assert_eq!(
        msg,
        PcbCanvasMsg::BoxSelected {
            from: rig.world(100.0, 80.0),
            to: rig.world(160.0, 140.0),
            mode: ClickMode::Replace,
        }
    );
    assert!(rig.canvas.marquee.get().is_none());
}

#[test]
fn the_threshold_is_four_pixels() {
    let mut rig = rig();
    rig.press(100.0, 80.0);
    rig.move_to(103.0, 80.0);
    assert!(rig.canvas.marquee.get().is_none());
    let msg = rig.release(103.0, 80.0).expect("a click");
    assert!(matches!(msg, PcbCanvasMsg::Clicked { .. }));

    rig.press(100.0, 80.0);
    rig.move_to(105.0, 80.0);
    assert!(rig.canvas.marquee.get().is_some());
    let msg = rig.release(105.0, 80.0).expect("a box");
    assert!(matches!(msg, PcbCanvasMsg::BoxSelected { .. }));
}

#[test]
fn shift_adds_and_ctrl_toggles() {
    let mut rig = rig();
    rig.mods(Modifiers::SHIFT);
    rig.press(50.0, 50.0);
    let msg = rig.release(50.0, 50.0).expect("a click");
    assert!(matches!(
        msg,
        PcbCanvasMsg::Clicked {
            mode: ClickMode::Add,
            ..
        }
    ));

    rig.mods(Modifiers::CTRL);
    rig.press(50.0, 50.0);
    rig.move_to(120.0, 120.0);
    let msg = rig.release(120.0, 120.0).expect("a box");
    assert!(matches!(
        msg,
        PcbCanvasMsg::BoxSelected {
            mode: ClickMode::Toggle,
            ..
        }
    ));
}

#[test]
fn a_release_outside_the_canvas_still_ends_the_box() {
    let mut rig = rig();
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    let msg = rig.release(520.0, 140.0).expect("a box");
    let PcbCanvasMsg::BoxSelected { to, .. } = msg else {
        panic!("not a box: {msg:?}");
    };
    assert_eq!(to, rig.world(520.0, 140.0));
}

#[test]
fn a_press_is_captured_and_a_press_outside_is_not() {
    let mut rig = rig();
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let inside =
        canvas::Program::update(&rig.canvas, &mut rig.state, &press, BOUNDS, at(10.0, 10.0));
    assert!(inside.is_some_and(|a| a.into_inner().2 == iced::event::Status::Captured));
    rig.release(10.0, 10.0);
    let outside =
        canvas::Program::update(&rig.canvas, &mut rig.state, &press, BOUNDS, at(900.0, 10.0));
    assert!(outside.is_none());
}

#[test]
fn the_readout_follows_the_pointer_during_a_box() {
    let mut rig = rig();
    rig.press(100.0, 80.0);
    let msg = rig.move_to(180.0, 140.0).expect("the pointer is published");
    let PcbCanvasMsg::PointerMoved { at, .. } = msg else {
        panic!("not a pointer move: {msg:?}");
    };
    assert_eq!(at, rig.world(180.0, 140.0));
}

#[test]
fn clearing_the_box_ends_the_gesture_without_a_selection() {
    let mut rig = rig();
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    rig.canvas.cancel_marquee();
    rig.move_to(170.0, 150.0);
    assert!(rig.release(170.0, 150.0).is_none());
    assert!(rig.canvas.marquee.get().is_none());

    // Released before any further move: still no box.
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    rig.canvas.cancel_marquee();
    assert!(rig.release(160.0, 140.0).is_none());
}

#[test]
fn leaving_the_window_or_a_new_press_drops_a_stale_press() {
    let mut rig = rig();
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    rig.send(Event::Mouse(mouse::Event::CursorLeft), at(-1.0, -1.0));
    assert!(rig.canvas.marquee.get().is_none());
    assert!(
        rig.move_to(200.0, 200.0)
            .is_some_and(|m| matches!(m, PcbCanvasMsg::PointerMoved { .. }))
    );
    assert!(
        rig.canvas.marquee.get().is_none(),
        "no box follows a free cursor"
    );

    // A release lost outside the window: the next press starts afresh.
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    rig.press(30.0, 30.0);
    assert!(rig.canvas.marquee.get().is_none());
    let msg = rig.release(30.0, 30.0).expect("a click");
    assert!(matches!(msg, PcbCanvasMsg::Clicked { .. }));
}

#[test]
fn a_press_closes_an_open_context_menu_even_if_it_never_releases() {
    let mut rig = rig();
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let action =
        canvas::Program::update(&rig.canvas, &mut rig.state, &press, BOUNDS, at(10.0, 10.0))
            .expect("a press inside the canvas is handled");
    assert!(matches!(
        action.into_inner().0,
        Some(Message::ContextMenu(ContextMenuMsg::Close))
    ));
    // The press ends by leaving the window: nothing else is published.
    assert!(
        rig.send(Event::Mouse(mouse::Event::CursorLeft), at(10.0, 10.0))
            .is_none()
    );
}

/// The canvas grabs a 2 mm disc around the board point under (100, 80).
fn grabbing(rig: &mut Rig) {
    let centre = pcb_geom::PointNm::from_mm(rig.world(100.0, 80.0), "test").unwrap();
    rig.canvas.set_grab(vec![pcb_geom::Shape::Disc {
        center: centre,
        radius: 2_000_000,
    }]);
}

#[test]
fn a_press_on_a_grabbed_footprint_drags_it_instead_of_a_box() {
    let mut rig = rig();
    grabbing(&mut rig);
    rig.press(100.0, 80.0);
    let (from, to) = (rig.world(100.0, 80.0), rig.world(160.0, 140.0));
    let msg = rig.move_to(160.0, 140.0);
    assert_eq!(msg, Some(PcbCanvasMsg::FootprintDragged { from, to }));
    assert!(rig.canvas.marquee.get().is_none(), "no box is drawn");
    let msg = rig.release(160.0, 140.0);
    assert_eq!(msg, Some(PcbCanvasMsg::FootprintDropped { from, to }));
}

#[test]
fn a_footprint_drag_cancelled_by_escape_drops_nothing() {
    let mut rig = rig();
    grabbing(&mut rig);
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    rig.canvas.cancel_marquee();
    assert_eq!(rig.release(160.0, 140.0), None);
}

#[test]
fn a_footprint_drag_that_leaves_the_canvas_is_cancelled() {
    let mut rig = rig();
    grabbing(&mut rig);
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    let left = rig.send(Event::Mouse(mouse::Event::CursorLeft), at(160.0, 140.0));
    assert_eq!(left, Some(PcbCanvasMsg::FootprintDragCancelled));
}

#[test]
fn a_click_on_a_grabbed_footprint_or_a_modified_press_still_selects() {
    let mut rig = rig();
    grabbing(&mut rig);
    rig.press(100.0, 80.0);
    let msg = rig.release(101.0, 80.0).expect("a click");
    assert!(matches!(msg, PcbCanvasMsg::Clicked { .. }), "{msg:?}");

    rig.mods(Modifiers::SHIFT);
    rig.press(100.0, 80.0);
    rig.move_to(160.0, 140.0);
    assert!(rig.canvas.marquee.get().is_some(), "Shift starts a box");
}

#[test]
fn a_right_click_that_does_not_pan_opens_the_board_menu() {
    let mut rig = rig();
    let right = |pressed: bool| {
        Event::Mouse(if pressed {
            mouse::Event::ButtonPressed(mouse::Button::Right)
        } else {
            mouse::Event::ButtonReleased(mouse::Button::Right)
        })
    };
    let mut run = |event: Event, x: f32| {
        canvas::Program::update(&rig.canvas, &mut rig.state, &event, BOUNDS, at(x, 50.0))
            .and_then(|action| action.into_inner().0)
    };
    run(right(true), 40.0);
    assert!(matches!(
        run(right(false), 40.0),
        Some(Message::ContextMenu(ContextMenuMsg::Show(x, y))) if x == 40.0 && y == 50.0
    ));
    // A right drag pans and opens nothing.
    run(right(true), 40.0);
    let moved = Event::Mouse(mouse::Event::CursorMoved {
        position: iced::Point::new(90.0, 50.0),
    });
    run(moved, 90.0);
    assert!(run(right(false), 90.0).is_none());
}
