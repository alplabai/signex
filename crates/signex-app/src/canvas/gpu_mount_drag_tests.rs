//! A move-drag on the GPU mount (#199 S3), with no GPU: the overlay's `update`
//! drives the drag, the programs' `draw` is read for the scenes they would hand
//! the GPU.

use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::{Event, mouse, window};
use signex_gfx::scene::Scene;
use signex_types::schematic::{SchDrawing, SelectedItem, SelectedKind};

use super::gpu_drag::SchematicDragShaderProgram;
use super::gpu_mount::SchematicShaderProgram;
use super::gpu_mount_tests::{fixture, slot_with};
use super::tests::test_prefs;
use super::*;
use crate::scene_shader::{ScenePrimitive, SchematicDragSurface, SchematicSurface};

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

struct Programs<'a> {
    overlay: SchematicCanvas<'a>,
    content: SchematicShaderProgram<'a>,
    drag: SchematicDragShaderProgram<'a>,
}

fn rig<R>(slot: &CanvasSlot, f: impl FnOnce(&Programs<'_>) -> R) -> R {
    rig_with(slot, &Default::default(), f)
}

fn rig_with<R>(
    slot: &CanvasSlot,
    overrides: &std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    f: impl FnOnce(&Programs<'_>) -> R,
) -> R {
    let filters = Default::default();
    let prefs = test_prefs(overrides, &filters);
    f(&Programs {
        overlay: SchematicCanvas::new(slot, prefs).gpu_overlay(),
        content: SchematicShaderProgram::new(slot, prefs),
        drag: SchematicDragShaderProgram::new(slot, prefs),
    })
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

fn content(p: &Programs<'_>) -> ScenePrimitive<SchematicSurface> {
    iced::widget::shader::Program::draw(&p.content, &(), at(0.0, 0.0), BOUNDS)
}

fn moving(p: &Programs<'_>) -> ScenePrimitive<SchematicDragSurface> {
    iced::widget::shader::Program::draw(&p.drag, &(), at(0.0, 0.0), BOUNDS)
}

/// A drag is live when the drag layer carries a moving set, not the one idle
/// generation.
fn drag_is_live(p: &Programs<'_>) -> bool {
    moving(p).generation != Some(super::gpu_drag::idle_generation())
}

fn is_blank(scene: &Scene) -> bool {
    scene.lines.is_empty()
        && scene.circles.is_empty()
        && scene.arcs.is_empty()
        && scene.polygons.is_empty()
        && scene.drawings.is_empty()
        && scene.drawings_front.is_empty()
        && scene.texts.is_empty()
}

/// Every primitive of `scenes`, one sorted `Debug` line each, so two scenes
/// compare as multisets whatever order they were emitted in.
fn multiset(scenes: &[&Scene]) -> Vec<String> {
    let mut all = Vec::new();
    for scene in scenes {
        let lines = [
            ("line", &scene.lines),
            ("overlay line", &scene.overlay_lines),
            ("erc line", &scene.erc_marker_lines),
        ];
        for (name, bucket) in lines {
            all.extend(bucket.iter().map(|x| format!("{name} {x:?}")));
        }
        let circles = [
            ("circle", &scene.circles),
            ("overlay circle", &scene.overlay_circles),
            ("erc circle", &scene.erc_marker_circles),
        ];
        for (name, bucket) in circles {
            all.extend(bucket.iter().map(|x| format!("{name} {x:?}")));
        }
        let polygons = [
            ("polygon", &scene.polygons),
            ("top polygon", &scene.top_polygons),
            ("overlay polygon", &scene.overlay_polygons),
            ("erc polygon", &scene.erc_marker_polygons),
        ];
        for (name, bucket) in polygons {
            all.extend(bucket.iter().map(|x| format!("{name} {x:?}")));
        }
        all.extend(scene.arcs.iter().map(|x| format!("arc {x:?}")));
        all.extend(scene.drawings.iter().map(|x| format!("drawing {x:?}")));
        all.extend(
            scene
                .drawings_front
                .iter()
                .map(|x| format!("front drawing {x:?}")),
        );
        all.extend(scene.texts.iter().map(|x| format!("text {x:?}")));
    }
    all.sort();
    all
}

/// The fixture with its wire shifted by `(dx, dy)`, as a scene: what a full
/// rebuild after the move would draw.
fn shifted_full_scene(dx: f64, dy: f64) -> Scene {
    let mut sheet = fixture();
    let wire = &mut sheet.wires[0];
    for end in [&mut wire.start, &mut wire.end] {
        end.x += dx;
        end.y += dy;
    }
    let slot = slot_with(&sheet);
    let overrides = Default::default();
    let filters = Default::default();
    let canvas = SchematicCanvas::new(&slot, test_prefs(&overrides, &filters));
    crate::schematic_runtime::build_schematic_scene(
        slot.active_snapshot().expect("snapshot"),
        &canvas.screen_transform(),
        &canvas.prefs.canvas_colors,
        crate::schematic_runtime::frame_local_bounds(BOUNDS),
        None,
        Default::default(),
    )
    .expect("scene")
}

#[test]
fn idle_the_drag_layer_emits_nothing_and_the_content_is_the_sheet() {
    let slot = slot_with_wire_selected();
    rig(&slot, |p| {
        let a = moving(p);
        let b = moving(p);
        assert!(is_blank(&a.scene), "an idle drag layer draws nothing");
        assert!(
            a.generation.is_some(),
            "and is uploaded once, not per frame"
        );
        assert_eq!(a.generation, b.generation);
        let full = content(p);
        assert!(!full.scene.lines.is_empty() && full.generation.is_some());
    });
}

#[test]
fn a_drag_moves_the_wire_from_the_static_scene_to_the_drag_layer() {
    let slot = slot_with_wire_selected();
    rig(&slot, |p| {
        let mut state = CanvasState::default();
        let full = content(p).scene.lines.len();

        press(&p.overlay, &mut state);
        assert!(!drag_is_live(p), "pressed, not yet dragging");

        drag_to(&p.overlay, &mut state, 150.0);
        assert!(state.move_dragging, "the fixture really starts a drag");
        assert!(drag_is_live(p));
        assert_eq!(moving(p).scene.lines.len(), 1, "the wire moves");
        assert_eq!(
            content(p).scene.lines.len(),
            full - 1,
            "and leaves the rest"
        );

        send(&p.overlay, &mut state, escape(), at(150.0, 60.0));
        assert!(!drag_is_live(p), "Esc ends it");
        assert_eq!(content(p).scene.lines.len(), full);
    });
}

#[test]
fn the_static_generation_holds_over_drag_frames_and_rebuilds_on_zoom() {
    let slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let held = rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        let first = content(p)
            .generation
            .expect("static scene has a generation");
        for x in [154.0, 158.0, 162.0, 170.0, 150.0] {
            drag_to(&p.overlay, &mut state, x);
            assert_eq!(content(p).generation, Some(first), "re-uploaded at x={x}");
            assert!(drag_is_live(p));
        }
        first
    });

    slot.camera_mut().scale = 10.0;
    rig(&slot, |p| {
        drag_to(&p.overlay, &mut state, 152.0);
        let zoomed = content(p).generation.expect("rebuilt");
        assert_ne!(zoomed, held, "a zoom step rebuilds the static scene");
        drag_to(&p.overlay, &mut state, 156.0);
        assert_eq!(content(p).generation, Some(zoomed), "and holds again");
    });
}

/// Static plus moving is the whole sheet with the wire moved, at the first
/// step of a float and at a larger shift.
#[test]
fn static_plus_moving_equal_the_shifted_full_scene() {
    // A float starts at delta 0: its origin is the press point.
    let slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), lifted);
    rig(&slot, |p| {
        drag_to(&p.overlay, &mut state, 120.0);
        // The first step of a float: a snapped delta, nearly nothing.
        let (dx, dy) = p.overlay.drag_delta(&state).expect("floating");
        assert!(dx.abs() < 1.0 && dy.abs() < 1.0);
        let (stat, mov) = (content(p), moving(p));
        assert_eq!(
            multiset(&[&stat.scene, &mov.scene]),
            multiset(&[&shifted_full_scene(dx, dy)]),
            "first step ({dx}, {dy})"
        );

        drag_to(&p.overlay, &mut state, 150.0);
        let (dx, dy) = p.overlay.drag_delta(&state).expect("dragging");
        assert!(dx.abs() > 1.0, "a real shift: {dx}");
        let (stat, mov) = (content(p), moving(p));
        assert_eq!(
            multiset(&[&stat.scene, &mov.scene]),
            multiset(&[&shifted_full_scene(dx, dy)]),
            "delta ({dx}, {dy})"
        );
    });
}

/// Neither shader layer reacts to input: a handler here would double every
/// click, as `stack!` hands events to all of them.
#[test]
fn the_shader_layers_never_emit_or_capture_events() {
    let slot = slot_with_wire_selected();
    rig(&slot, |p| {
        let events = [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Event::Mouse(mouse::Event::CursorMoved {
                position: iced::Point::new(130.0, 60.0),
            }),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            escape(),
            Event::Window(window::Event::Focused),
        ];
        for event in &events {
            let cursor = at(120.0, 60.0);
            assert!(
                iced::widget::shader::Program::update(&p.content, &mut (), event, BOUNDS, cursor)
                    .is_none(),
                "the content shader reacted to {event:?}"
            );
            assert!(
                iced::widget::shader::Program::update(&p.drag, &mut (), event, BOUNDS, cursor)
                    .is_none(),
                "the drag shader reacted to {event:?}"
            );
        }
    });
}

fn redraw_requested() -> Event {
    Event::Window(window::Event::RedrawRequested(std::time::Instant::now()))
}

/// The runtime's real order: iced's `Canvas::update` forwards every event, the
/// `RedrawRequested` that precedes each draw included, to `Program::update`, so
/// the overlay re-judges the gesture right before the shaders are read. No
/// event of the float's own needs to arrive in between.
#[test]
fn a_redraw_request_after_an_app_message_leaves_no_blank_sheet() {
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), lifted);
    let full = rig(&slot, |p| {
        let full = content(p).scene.lines.len();
        drag_to(&p.overlay, &mut state, 150.0);
        send(&p.overlay, &mut state, redraw_requested(), at(150.0, 60.0));
        assert!(drag_is_live(p), "a live float survives a redraw request");
        full
    });

    // Undo bumps the epoch; the frame that follows starts with RedrawRequested.
    slot.move_epoch += 1;
    rig(&slot, |p| {
        send(&p.overlay, &mut state, redraw_requested(), at(150.0, 60.0));
        assert!(!drag_is_live(p), "the drag layer empties");
        assert_eq!(content(p).scene.lines.len(), full, "no blank sheet");
    });

    // Delete empties the selection, epoch untouched.
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), lifted);
    rig(&slot, |p| {
        drag_to(&p.overlay, &mut state, 150.0);
        assert!(drag_is_live(p));
    });
    slot.selected.clear();
    rig(&slot, |p| {
        send(&p.overlay, &mut state, redraw_requested(), at(150.0, 60.0));
        assert!(!drag_is_live(p));
        assert_eq!(content(p).scene.lines.len(), full, "no blank sheet");
    });
}

/// A float whose pick has not landed holds the selection it was lifted from
/// (`before`); the app then writes the lifted selection. Both are the same
/// float, so the flip must neither end it nor leave a second copy drawn: after
/// the redraw request the item is on the drag layer once and out of the static
/// layer.
#[test]
fn a_before_to_lifted_selection_flip_draws_the_item_once() {
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let lifted = slot.selected.clone();
    let before = vec![SelectedItem::new(
        fixture().wires[0].uuid,
        SelectedKind::Junction,
    )];
    slot.selected = before.clone();
    state.start_floating_move((12.5, 5.0), slot.move_epoch, lifted.clone(), before);
    let full = rig(&slot, |p| content(p).scene.lines.len());
    rig(&slot, |p| {
        drag_to(&p.overlay, &mut state, 150.0);
        send(&p.overlay, &mut state, redraw_requested(), at(150.0, 60.0));
    });

    // The pick lands: the selection becomes the lifted one.
    slot.selected = lifted;
    rig(&slot, |p| {
        send(&p.overlay, &mut state, redraw_requested(), at(150.0, 60.0));
        assert!(drag_is_live(p), "the flip does not end the float");
        assert_eq!(moving(p).scene.lines.len(), 1, "the wire is drawn moving");
        assert_eq!(
            content(p).scene.lines.len(),
            full - 1,
            "and is gone from the static layer: no double draw"
        );
    });
}

/// A tab switch hands the slot a canvas state with no drag in it; the redraw
/// request that precedes the next draw ends the published drag.
#[test]
fn a_fresh_canvas_state_ends_a_published_drag_on_the_next_redraw() {
    let slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        assert!(drag_is_live(p));
        let mut fresh = CanvasState::default();
        send(&p.overlay, &mut fresh, redraw_requested(), at(150.0, 60.0));
        assert!(!drag_is_live(p));
    });
}

/// Ending a drag with an event that publishes nothing must still request a
/// redraw, or the moved item stays gone until the mouse moves.
#[test]
fn ending_a_drag_requests_a_redraw() {
    let slot = slot_with_wire_selected();
    rig(&slot, |p| {
        let mut state = CanvasState::default();
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        let action = send(&p.overlay, &mut state, escape(), at(150.0, 60.0))
            .expect("Esc ends the drag with an action");
        let (message, redraw, _) = action.into_inner();
        assert!(
            message.is_some() || !matches!(redraw, window::RedrawRequest::Wait),
            "the layers change hands with no redraw to show it"
        );
    });
}

/// CPU mode has no shaders to inform, so it must not do the work.
#[test]
fn a_cpu_canvas_publishes_no_drag_state() {
    let slot = slot_with_wire_selected();
    let overrides = Default::default();
    let filters = Default::default();
    let prefs = test_prefs(&overrides, &filters);
    let cpu = SchematicCanvas::new(&slot, prefs);
    let drag = SchematicDragShaderProgram::new(&slot, prefs);
    let mut state = CanvasState::default();
    press(&cpu, &mut state);
    drag_to(&cpu, &mut state, 150.0);
    assert!(state.move_dragging);
    let primitive: ScenePrimitive<SchematicDragSurface> =
        iced::widget::shader::Program::draw(&drag, &(), at(0.0, 0.0), BOUNDS);
    assert!(primitive.generation.is_some(), "no live drag was published");
}

fn with_second_wire_selected(
    slot: &mut CanvasSlot,
    sheet: &signex_types::schematic::SchematicSheet,
) {
    slot.selected
        .push(SelectedItem::new(sheet.wires[1].uuid, SelectedKind::Wire));
}

/// The static layer follows the document it was built from: a new revision, a
/// changed skip set, or the render cache being replaced each rebuild it.
#[test]
fn a_new_revision_or_skip_set_during_a_drag_gives_a_new_static_generation() {
    let sheet = fixture();
    let mut slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    let first = rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        let first = content(p).generation.expect("generation");
        drag_to(&p.overlay, &mut state, 154.0);
        assert_eq!(content(p).generation, Some(first), "unchanged: held");
        first
    });

    // The document moved on in place: the engine revision is new.
    slot.render_cache =
        Some(crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&sheet, 2));
    let second = rig(&slot, |p| {
        drag_to(&p.overlay, &mut state, 158.0);
        let second = content(p).generation.expect("generation");
        assert_ne!(second, first, "a new revision rebuilds the static scene");
        second
    });

    // A second item joins the selection: the ids the static layer leaves out
    // change.
    with_second_wire_selected(&mut slot, &sheet);
    rig(&slot, |p| {
        drag_to(&p.overlay, &mut state, 162.0);
        let third = content(p).generation.expect("generation");
        assert_ne!(third, second, "a new skip set rebuilds the static scene");
    });
}

/// Replacing the render cache replaces the document: nothing of the old drag's
/// static layer may survive, even at the same revision.
#[test]
fn replacing_the_render_cache_drops_the_static_layer() {
    for revision in [0, 1] {
        let sheet = fixture();
        let mut slot = slot_with(&sheet);
        slot.selected = vec![SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire)];
        slot.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&sheet, revision),
        ));
        let mut state = CanvasState::default();
        let before = rig(&slot, |p| {
            press(&p.overlay, &mut state);
            drag_to(&p.overlay, &mut state, 150.0);
            content(p).scene.lines.len()
        });

        // Another document, same revision, without the second wire.
        let mut other = sheet.clone();
        other.wires.pop();
        slot.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&other, revision),
        ));
        slot.selected = vec![SelectedItem::new(other.wires[0].uuid, SelectedKind::Wire)];
        let after = rig(&slot, |p| {
            let mut state = CanvasState::default();
            press(&p.overlay, &mut state);
            drag_to(&p.overlay, &mut state, 150.0);
            assert!(drag_is_live(p));
            content(p).scene.lines.len()
        });
        assert_eq!(
            after,
            before - 1,
            "revision {revision}: a stale static layer"
        );
    }
}

/// The moving scene depends on more than the camera.
#[test]
fn the_moving_scene_is_rebuilt_when_its_styling_changes() {
    let slot = slot_with_wire_selected();
    let wire = slot.selected[0].uuid;
    let mut state = CanvasState::default();
    let plain = rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        moving(p).scene.lines[0].color
    });
    let recolour = signex_types::theme::Color {
        r: 10,
        g: 200,
        b: 30,
        a: 255,
    };
    let overrides = std::collections::HashMap::from([(wire, recolour)]);
    let styled = rig_with(&slot, &overrides, |p| moving(p).scene.lines[0].color);
    assert_ne!(styled, plain, "the moving wire keeps its old colour");
}

/// Events that change nothing about the drag leave its frame, and the scene
/// built from it, as they were. Plain shifts are rebuilt per frame otherwise,
/// so this runs without the rubber-banded ghost, which hands back one frame.
#[test]
fn an_unchanged_drag_keeps_its_frame_across_events() {
    let slot = slot_with_wire_selected();
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.move_keeps_wires = false;
    let overlay = SchematicCanvas::new(&slot, prefs).gpu_overlay();
    let mut state = CanvasState::default();
    press(&overlay, &mut state);
    drag_to(&overlay, &mut state, 150.0);
    let frame = overlay.live_drag().expect("live");
    // Same snapped delta, a focus event: no new frame.
    drag_to(&overlay, &mut state, 150.5);
    send(
        &overlay,
        &mut state,
        Event::Window(window::Event::Focused),
        at(150.0, 60.0),
    );
    let again = overlay.live_drag().expect("live");
    assert!(std::rc::Rc::ptr_eq(&frame.moving, &again.moving));
    // A new delta is a new frame.
    drag_to(&overlay, &mut state, 170.0);
    let moved = overlay.live_drag().expect("live");
    assert!(!std::rc::Rc::ptr_eq(&frame.moving, &moved.moving));
}

/// Revision `0` is "unknown": the document may have changed under it, so a
/// static layer built at it is never reused.
#[test]
fn an_unknown_revision_never_reuses_the_static_layer() {
    let sheet = fixture();
    let mut slot = slot_with(&sheet);
    slot.selected = vec![SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire)];
    slot.render_cache =
        Some(crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&sheet, 0));
    let mut state = CanvasState::default();
    let before = rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        content(p).scene.lines.len()
    });
    // The same cache is refreshed in place, still reporting no revision.
    let mut other = sheet.clone();
    other.wires.pop();
    slot.render_cache =
        Some(crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&other, 0));
    rig(&slot, |p| {
        drag_to(&p.overlay, &mut state, 158.0);
        assert_eq!(
            content(p).scene.lines.len(),
            before - 1,
            "a stale static layer"
        );
    });
}

/// The moving scene is built for one camera: a pan of at least 0.01 px moves
/// it, and a smaller one does not.
#[test]
fn a_pan_of_a_hundredth_of_a_pixel_rebuilds_the_moving_scene() {
    let slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        let built = moving(p).scene;
        slot.camera_mut().offset.x += 0.001;
        assert!(
            std::sync::Arc::ptr_eq(&built, &moving(p).scene),
            "a sub-hundredth pan reuses the scene"
        );
        slot.camera_mut().offset.x += 0.02;
        assert!(
            !std::sync::Arc::ptr_eq(&built, &moving(p).scene),
            "a 0.02 px pan must rebuild it"
        );
    });
}

/// The static scene is built a viewport past the window on every side: a pan
/// inside that keeps its generation, a pan past it builds a new one.
#[test]
fn a_pan_past_the_margin_rebuilds_the_static_scene() {
    let slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        let built = content(p).generation.expect("static generation");
        slot.camera_mut().offset.x += 100.0;
        assert_eq!(content(p).generation, Some(built), "inside the margin");
        slot.camera_mut().offset.x += 300.0;
        let rebuilt = content(p).generation.expect("rebuilt");
        assert_ne!(rebuilt, built, "past the margin");
    });
}

/// After Esc the content shader draws the whole sheet again from the same
/// scene it had before the drag, so the GPU has nothing to re-upload.
#[test]
fn after_esc_the_content_shader_serves_the_pre_drag_generation() {
    let slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    rig(&slot, |p| {
        let before = content(p);
        let generation = before.generation.expect("generation");
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        assert_ne!(content(p).generation, Some(generation), "static scene");
        send(&p.overlay, &mut state, escape(), at(150.0, 60.0));
        let after = content(p);
        assert_eq!(after.generation, Some(generation));
        assert_eq!(after.scene.lines.len(), before.scene.lines.len());
    });
}

/// Every idle drag layer carries the one idle generation; a dragging slot's
/// layer carries none, and no content scene is given that number.
#[test]
fn a_dragging_slot_and_an_idle_one_do_not_collide_on_the_idle_generation() {
    let dragging = slot_with_wire_selected();
    let idle = slot_with_wire_selected();
    let mut state = CanvasState::default();
    rig(&dragging, |a| {
        rig(&idle, |b| {
            press(&a.overlay, &mut state);
            drag_to(&a.overlay, &mut state, 150.0);
            let (live, resting) = (moving(a), moving(b));
            let idle_generation = resting.generation.expect("idle generation");
            let live_generation = live.generation.expect("a live drag has a generation");
            assert_ne!(live_generation, idle_generation, "and not the idle one");
            assert!(!is_blank(&live.scene));
            assert!(is_blank(&resting.scene));
            assert_ne!(content(a).generation, Some(idle_generation));
            assert_ne!(content(b).generation, Some(idle_generation));
            assert_eq!(moving(b).generation, Some(idle_generation), "and holds");
        });
    });
}

/// A drawing placed above the circuit stays above it while it is dragged, in
/// the moving layer, and is not in the static layer's front bucket (#1583).
#[test]
fn a_dragged_above_drawing_keeps_its_placement_in_both_layers() {
    use signex_types::schematic::DrawingPlacement;
    let mut sheet = fixture();
    let SchDrawing::Rect { uuid, .. } = sheet.drawings[0] else {
        panic!("the fixture's first drawing is a rectangle");
    };
    sheet
        .drawing_placements
        .insert(uuid, DrawingPlacement::Above);
    let mut slot = slot_with(&sheet);
    slot.selected = vec![SelectedItem::new(uuid, SelectedKind::Drawing)];
    rig(&slot, |p| {
        let mut state = CanvasState::default();
        assert_eq!(content(p).scene.drawings_front.len(), 1, "idle: above");

        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        send(&p.overlay, &mut state, press, at(208.0, 116.0));
        drag_to(&p.overlay, &mut state, 240.0);
        assert!(state.move_dragging);

        let moved = moving(p);
        assert_eq!(moved.scene.drawings_front.len(), 1, "moving layer: above");
        assert!(moved.scene.drawings.is_empty());
        assert!(
            content(p).scene.drawings_front.is_empty(),
            "the static layer no longer holds it"
        );
    });
}

/// A moved drawing leaves the static layer's ordered bucket for the moving
/// layer's, so the drag layer carries the bucket too (#1579).
#[test]
fn a_dragged_drawing_moves_from_the_static_bucket_to_the_moving_one() {
    let sheet = fixture();
    let mut slot = slot_with(&sheet);
    // The fixture's rectangle drawing, outlined from world (22, 12) to
    // (25, 15): its top edge at world (23.5, 12.0) is pixel (208, 116).
    let SchDrawing::Rect { uuid, .. } = sheet.drawings[0] else {
        panic!("the fixture's first drawing is a rectangle");
    };
    slot.selected = vec![SelectedItem::new(uuid, SelectedKind::Drawing)];
    rig(&slot, |p| {
        let mut state = CanvasState::default();
        let all = content(p).scene.drawings.len();
        assert!(all > 0, "the fixture draws drawings");

        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        send(&p.overlay, &mut state, press, at(208.0, 116.0));
        drag_to(&p.overlay, &mut state, 240.0);
        assert!(state.move_dragging, "pressing the drawing starts a drag");

        assert!(
            !moving(p).scene.drawings.is_empty(),
            "the moved drawing is on the moving layer"
        );
        assert!(
            content(p).scene.drawings.len() < all,
            "and no longer on the static one"
        );
    });
}

/// The moving scene is keyed by its own generation: a frame with nothing moved
/// keeps it, so the GPU uploads nothing, and a cursor step takes a new one.
#[test]
fn the_moving_generation_holds_when_nothing_moved_and_changes_when_it_does() {
    let slot = slot_with_wire_selected();
    let mut state = CanvasState::default();
    rig(&slot, |p| {
        press(&p.overlay, &mut state);
        drag_to(&p.overlay, &mut state, 150.0);
        let first = moving(p).generation.expect("a live drag has a generation");

        assert_eq!(moving(p).generation, Some(first), "a still frame");
        assert_eq!(moving(p).generation, Some(first), "and another");

        drag_to(&p.overlay, &mut state, 170.0);
        let stepped = moving(p).generation.expect("generation");
        assert_ne!(stepped, first, "a cursor step rebuilds and re-keys");
    });
}
