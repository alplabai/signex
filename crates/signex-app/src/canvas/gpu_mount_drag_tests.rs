//! How the GPU mount behaves around a move-drag (#199 S2), with no GPU: the
//! overlay's `update` drives the drag, the shader program's `draw` is read for
//! the scene it would hand the GPU.

use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::{Event, mouse, window};
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::gpu_mount::SchematicShaderProgram;
use super::gpu_mount_tests::{fixture, slot_with};
use super::tests::test_prefs;
use super::*;
use crate::scene_shader::{ScenePrimitive, SchematicSurface};

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 260.0,
    height: 200.0,
};

/// A slot over the fixture with its wire selected: pressing on the wire
/// (world 12.5, 5.0 = pixel 120, 60) prepares a drag of it.
fn slot_with_wire_selected() -> CanvasSlot {
    let sheet = fixture();
    let mut slot = slot_with(&sheet);
    slot.selected = vec![SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire)];
    slot
}

fn at(x: f32, y: f32) -> mouse::Cursor {
    mouse::Cursor::Available(iced::Point::new(x, y))
}

fn rig<R>(
    slot: &CanvasSlot,
    f: impl FnOnce(&SchematicCanvas<'_>, &SchematicShaderProgram<'_>) -> R,
) -> R {
    let overrides = Default::default();
    let filters = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    let overlay = SchematicCanvas::new(slot, prefs).gpu_overlay();
    let shader = SchematicShaderProgram::new(slot, prefs);
    f(&overlay, &shader)
}

fn send(
    canvas: &SchematicCanvas<'_>,
    state: &mut CanvasState,
    event: Event,
    cursor: mouse::Cursor,
) -> Option<canvas::Action<Message>> {
    canvas::Program::update(canvas, state, &event, BOUNDS, cursor)
}

fn press(canvas: &SchematicCanvas<'_>, state: &mut CanvasState) {
    let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    send(canvas, state, event, at(120.0, 60.0));
}

fn drag_to(canvas: &SchematicCanvas<'_>, state: &mut CanvasState, x: f32) {
    let event = Event::Mouse(mouse::Event::CursorMoved {
        position: iced::Point::new(x, 60.0),
    });
    send(canvas, state, event, at(x, 60.0));
}

fn escape() -> Event {
    let key = Key::Named(Named::Escape);
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::default(),
        text: None,
        repeat: false,
    })
}

fn primitive(shader: &SchematicShaderProgram<'_>) -> ScenePrimitive<SchematicSurface> {
    iced::widget::shader::Program::draw(shader, &(), at(0.0, 0.0), BOUNDS)
}

fn is_full(primitive: &ScenePrimitive<SchematicSurface>) -> bool {
    !primitive.scene.lines.is_empty() && primitive.generation.is_some()
}

fn is_empty(primitive: &ScenePrimitive<SchematicSurface>) -> bool {
    primitive.scene.lines.is_empty()
        && primitive.scene.arcs.is_empty()
        && primitive.scene.polygons.is_empty()
        && primitive.scene.texts.is_empty()
        && primitive.generation.is_none()
}

#[test]
fn the_shader_yields_nothing_while_a_drag_draws_on_the_cpu_and_the_sheet_after() {
    let slot = slot_with_wire_selected();
    rig(&slot, |overlay, shader| {
        let mut state = CanvasState::default();
        assert!(is_full(&primitive(shader)), "idle: the GPU draws the sheet");

        press(overlay, &mut state);
        assert!(is_full(&primitive(shader)), "pressed, not yet dragging");

        drag_to(overlay, &mut state, 150.0);
        assert!(state.move_dragging, "the fixture really starts a drag");
        assert!(
            is_empty(&primitive(shader)),
            "dragging: CPU draws, GPU none"
        );

        send(overlay, &mut state, escape(), at(150.0, 60.0));
        assert!(!state.move_dragging);
        assert!(is_full(&primitive(shader)), "Esc: the GPU draws again");
    });
}

/// A click-click float made stale by an app message (Delete, Undo: it bumps
/// `move_epoch` or edits the selection) is no drag any more. The overlay sees
/// that at draw time; the shader must too, or only paper shows until the next
/// mouse event.
#[test]
fn a_float_made_stale_by_an_app_message_draws_the_full_scene() {
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), lifted);
    rig(&slot, |overlay, shader| {
        drag_to(overlay, &mut state, 150.0);
        assert!(
            is_empty(&primitive(shader)),
            "a live float draws on the CPU"
        );
    });

    // The keymap message lands: no further canvas event has run yet.
    slot.move_epoch += 1;
    rig(&slot, |overlay, shader| {
        assert!(overlay.float_is_stale(&state));
        assert!(is_full(&primitive(shader)), "stale float: the GPU draws");
    });

    // Same for an edited selection with the epoch untouched.
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), lifted);
    rig(&slot, |overlay, shader| {
        drag_to(overlay, &mut state, 150.0);
        assert!(is_empty(&primitive(shader)));
    });
    slot.selected.clear();
    rig(&slot, |overlay, shader| {
        assert!(overlay.float_is_stale(&state));
        assert!(
            is_full(&primitive(shader)),
            "selection edited: the GPU draws"
        );
    });
}

fn redraw_requested() -> Event {
    Event::Window(window::Event::RedrawRequested(std::time::Instant::now()))
}

/// The runtime's real order: iced's `Canvas::update` forwards every event, the
/// `RedrawRequested` that precedes each draw included, to `Program::update`, so
/// the overlay re-judges the gesture right before the shader is read. No
/// event of the float's own needs to arrive in between.
#[test]
fn a_redraw_request_after_an_app_message_leaves_no_blank_sheet() {
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), lifted);
    rig(&slot, |overlay, shader| {
        drag_to(overlay, &mut state, 150.0);
        send(overlay, &mut state, redraw_requested(), at(150.0, 60.0));
        assert!(
            is_empty(&primitive(shader)),
            "a live float survives a redraw request"
        );
    });

    // Undo bumps the epoch; the frame that follows starts with RedrawRequested.
    slot.move_epoch += 1;
    rig(&slot, |overlay, shader| {
        send(overlay, &mut state, redraw_requested(), at(150.0, 60.0));
        assert!(is_full(&primitive(shader)), "stale float: no blank sheet");
    });

    // Delete empties the selection, epoch untouched.
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), lifted);
    rig(&slot, |overlay, shader| {
        drag_to(overlay, &mut state, 150.0);
        assert!(is_empty(&primitive(shader)));
    });
    slot.selected.clear();
    rig(&slot, |overlay, shader| {
        send(overlay, &mut state, redraw_requested(), at(150.0, 60.0));
        assert!(is_full(&primitive(shader)), "deleted float: no blank sheet");
    });
}

/// The canvas leaving the tree mid-drag (a tab switch) takes its state with
/// it; nothing is left to end the drag, so the cache clear must.
#[test]
fn clearing_the_content_cache_ends_a_drag_for_the_shader() {
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    rig(&slot, |overlay, shader| {
        press(overlay, &mut state);
        drag_to(overlay, &mut state, 150.0);
        assert!(is_empty(&primitive(shader)));
    });
    slot.clear_content_cache();
    rig(&slot, |_, shader| {
        assert!(is_full(&primitive(shader)));
    });
}

/// Ending a drag with an event that publishes nothing must still request a
/// redraw, or the blank frame stays until the mouse moves.
#[test]
fn ending_a_drag_requests_a_redraw() {
    let slot = slot_with_wire_selected();
    rig(&slot, |overlay, _| {
        let mut state = CanvasState::default();
        press(overlay, &mut state);
        drag_to(overlay, &mut state, 150.0);
        let action = send(overlay, &mut state, escape(), at(150.0, 60.0))
            .expect("Esc ends the drag with an action");
        let (message, redraw, _) = action.into_inner();
        assert!(
            message.is_some() || !matches!(redraw, window::RedrawRequest::Wait),
            "the shader turns back on with no redraw to show it"
        );
    });
}

/// CPU mode has no shader to inform, so it must not do the work.
#[test]
fn a_cpu_canvas_publishes_no_drag_state() {
    let slot = slot_with_wire_selected();
    let overrides = Default::default();
    let filters = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    let cpu = SchematicCanvas::new(&slot, prefs);
    let shader = SchematicShaderProgram::new(&slot, prefs);
    let mut state = CanvasState::default();
    press(&cpu, &mut state);
    drag_to(&cpu, &mut state, 150.0);
    assert!(state.move_dragging);
    assert!(is_full(&primitive(&shader)), "CPU mode never publishes");
}
