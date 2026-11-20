//! The end grips of a selected wire: a press on one arms a drag of that end
//! alone, and the drop publishes `WireEndDragged` with the end's new place.

use super::move_pick::tests::{BOUNDS, screen, slot_with_wire};
use super::tests::test_prefs;
use super::*;
use iced::widget::canvas::Program as _;
use signex_types::schematic::{SelectedItem, SelectedKind};

/// Deliver `event` at `at` the way the runtime does; the published message.
fn send(
    slot: &CanvasSlot,
    state: &mut CanvasState,
    event: Event,
    at: iced::Point,
) -> Option<Message> {
    let all: std::collections::HashSet<_> = crate::active_bar::SelectionFilter::ALL
        .iter()
        .copied()
        .collect();
    let overrides = std::collections::HashMap::new();
    let view = SchematicCanvas::new(slot, test_prefs(&overrides, &all));
    let action = view.update(state, &event, BOUNDS, mouse::Cursor::Available(at))?;
    action.into_inner().0
}

fn left(pressed: bool) -> Event {
    let button = mouse::Button::Left;
    Event::Mouse(if pressed {
        mouse::Event::ButtonPressed(button)
    } else {
        mouse::Event::ButtonReleased(button)
    })
}

/// The wire (10,10)-(30,10) mm, selected.
fn selected_wire_slot() -> CanvasSlot {
    let mut slot = slot_with_wire();
    let wire = slot.active_snapshot().expect("snapshot").wires[0].uuid;
    slot.selected = vec![SelectedItem::new(wire, SelectedKind::Wire)];
    slot
}

#[test]
fn a_press_on_a_selected_wires_end_takes_its_grip() {
    let slot = selected_wire_slot();
    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 30.0, 10.0));
    assert_eq!(state.held_grip().map(|grip| grip.start), Some(false));

    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 10.0, 10.0));
    assert_eq!(state.held_grip().map(|grip| grip.start), Some(true));

    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 20.0, 10.0));
    assert!(state.held_grip().is_none(), "mid-wire moves the wire");
}

#[test]
fn an_unselected_wire_has_no_grips() {
    let slot = slot_with_wire();
    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 30.0, 10.0));
    assert!(state.held_grip().is_none());
}

#[test]
fn dragging_a_grip_drops_that_end_on_the_grid_step() {
    let slot = selected_wire_slot();
    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 30.0, 10.0));
    // Ten 1.27 mm grid steps to the right.
    let to = screen(&slot, 42.7, 10.0);
    let moved = Event::Mouse(mouse::Event::CursorMoved { position: to });
    send(&slot, &mut state, moved, to);
    let dropped = send(&slot, &mut state, left(false), to);
    let Some(Message::CanvasEvent(CanvasEvent::WireEndDragged { start, x, y, .. })) = &dropped
    else {
        panic!("the release did not drop the grip: {dropped:?}");
    };
    assert!(!*start, "the end, not the start");
    assert!(
        (x - 42.7).abs() < 1e-6 && (y - 10.0).abs() < 1e-6,
        "({x}, {y})"
    );
    assert!(state.held_grip().is_none(), "the drop ends the grip");
}

#[test]
fn a_right_click_cancels_a_grip_drag() {
    let slot = selected_wire_slot();
    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 30.0, 10.0));
    let to = screen(&slot, 42.7, 10.0);
    let moved = Event::Mouse(mouse::Event::CursorMoved { position: to });
    send(&slot, &mut state, moved, to);
    let right = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
    send(&slot, &mut state, right, to);
    assert!(state.held_grip().is_none());
    let released = send(&slot, &mut state, left(false), to);
    assert!(released.is_none(), "nothing dropped");
}
