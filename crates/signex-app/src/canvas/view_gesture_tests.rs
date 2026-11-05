//! Which canvas events are the user acting on the view (#1994): a wheel zoom
//! and a pan publish `ViewMoved`, a hover publishes the cursor events.

use super::tests::test_prefs;
use super::*;
use iced::Event;
use iced::widget::canvas::Program as _;

fn bounds() -> Rectangle {
    Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0))
}

/// The canvas event an input event publishes, if it publishes one.
fn published(
    view: &SchematicCanvas<'_>,
    state: &mut CanvasState,
    event: Event,
    at: iced::Point,
) -> Option<CanvasEvent> {
    let action = view.update(state, &event, bounds(), mouse::Cursor::Available(at))?;
    match action.into_inner().0? {
        Message::CanvasEvent(event) => Some(event),
        _ => None,
    }
}

fn moved(to: iced::Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: to })
}

#[test]
fn a_wheel_zoom_is_a_user_gesture() {
    let slot = CanvasSlot::new();
    let (overrides, filters) = Default::default();
    let view = SchematicCanvas::new(&slot, test_prefs(&overrides, &filters));
    let event = Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
    });
    let got = published(
        &view,
        &mut CanvasState::default(),
        event,
        iced::Point::new(300.0, 200.0),
    );
    assert!(matches!(got, Some(CanvasEvent::ViewMoved)), "{got:?}");
    assert!(got.is_some_and(|e| e.is_user_gesture()));
}

#[test]
fn a_pan_drag_is_a_user_gesture_but_a_hover_is_not() {
    let slot = CanvasSlot::new();
    let (overrides, filters) = Default::default();
    let view = SchematicCanvas::new(&slot, test_prefs(&overrides, &filters));
    let mut state = CanvasState::default();
    let start = iced::Point::new(100.0, 100.0);

    let hover = published(&view, &mut state, moved(start), start);
    assert!(
        matches!(hover, Some(CanvasEvent::CursorAt { .. })),
        "{hover:?}"
    );
    assert!(!hover.is_some_and(|e| e.is_user_gesture()));

    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
    let _ = view.update(
        &mut state,
        &press,
        bounds(),
        mouse::Cursor::Available(start),
    );
    // The first moving frame closes the context menu; the next ones pan.
    let first = iced::Point::new(120.0, 120.0);
    let _ = view.update(
        &mut state,
        &moved(first),
        bounds(),
        mouse::Cursor::Available(first),
    );
    let second = iced::Point::new(150.0, 140.0);
    let pan = published(&view, &mut state, moved(second), second);
    assert!(matches!(pan, Some(CanvasEvent::ViewMoved)), "{pan:?}");
}

#[test]
fn a_pixel_scroll_pans_with_trackpad_pan_on_and_zooms_while_ctrl_is_held() {
    let slot = CanvasSlot::new();
    let (overrides, filters) = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.trackpad_pan = true;
    let view = SchematicCanvas::new(&slot, prefs);
    let at = iced::Point::new(300.0, 200.0);
    let scroll = || {
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 20.0, y: 50.0 },
        })
    };

    let before = slot.camera().clone();
    let mut state = CanvasState::default();
    assert!(published(&view, &mut state, scroll(), at).is_some());
    let panned = slot.camera().clone();
    assert_eq!(panned.scale, before.scale);
    assert_ne!(panned.offset, before.offset);

    // Ctrl alone (not Cmd) sets the zoom modifier, so the scroll zooms.
    let mods = Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
        iced::keyboard::Modifiers::CTRL,
    ));
    let _ = view.update(&mut state, &mods, bounds(), mouse::Cursor::Available(at));
    assert!(state.zoom_modifier);
    assert!(published(&view, &mut state, scroll(), at).is_some());
    assert_ne!(slot.camera().scale, panned.scale);
}
