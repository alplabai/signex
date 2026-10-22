//! Regression tests (#170) that drive the REAL symbol-editor input
//! chain: iced event -> `SymbolCanvas` `Program::update` ->
//! `CanvasAction` -> `SymbolEditorMsg` -> `apply_symbol_primitive_edit`.
//! Nothing is mocked: the rig builds the same canvas the view builds
//! and applies the same message the dispatcher would.

use super::*;
use crate::app::SymbolEditorState;
use crate::library::editor::standalone::symbol::symbol_action_to_primitive_msg;
use crate::library::editor::symbol::updates::apply_symbol_primitive_edit;
use iced::event::Event;
use iced::{Point, Rectangle, keyboard, mouse};
use signex_library::{PinOrientation, Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind};
use std::path::PathBuf;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 900.0,
    height: 600.0,
};

struct Rig {
    editor: SymbolEditorState,
    cstate: CanvasState,
}

impl Rig {
    fn new(symbol: Symbol) -> Self {
        let mut editor =
            SymbolEditorState::new(PathBuf::from("t.snxsym"), SymbolFile::from_symbol(symbol));
        // A settled camera: the tests are about hit-testing, not the
        // queued first-view placement.
        editor.pending_view = None;
        Self {
            editor,
            cstate: CanvasState::default(),
        }
    }

    fn screen(&self, x: f64, y: f64) -> Point {
        let cam = &self.editor.camera;
        Point::new(
            cam.offset.x + x as f32 * cam.scale,
            cam.offset.y - y as f32 * cam.scale,
        )
    }

    /// Feed one event through the canvas and apply what it publishes.
    /// Returns whether the canvas claimed the event (produced an action).
    fn send(&mut self, event: Event, at: Option<(f64, f64)>) -> bool {
        let cursor = at.map_or(mouse::Cursor::Unavailable, |(x, y)| {
            mouse::Cursor::Available(self.screen(x, y))
        });
        let editor = &self.editor;
        let canvas = SymbolCanvas::new(
            editor.primitive(),
            editor.selected.clone(),
            editor.tool,
            &editor.polygon_vertices,
            editor.active_part,
            false,
            &editor.camera,
            1.27,
            true,
            crate::render_config::GridStyle::default(),
            true,
            Color::WHITE,
            Color::from_rgb(0.2, 0.4, 0.9),
            Color::BLACK,
            Color::BLACK,
            Color::BLACK,
        )
        .with_selection_filter(editor.selection_filter)
        .with_view_pending(editor.pending_view.is_some());
        let action = canvas::Program::update(&canvas, &mut self.cstate, &event, BOUNDS, cursor);
        let claimed = action.is_some();
        if let Some((Some(published), _, _)) = action.map(canvas::Action::into_inner) {
            apply_symbol_primitive_edit(
                &mut self.editor,
                symbol_action_to_primitive_msg(published),
            );
        }
        claimed
    }

    fn press(&mut self, at: (f64, f64)) {
        self.send(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Some(at),
        );
    }

    fn release(&mut self, at: (f64, f64)) {
        self.send(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Some(at),
        );
    }

    fn moved(&mut self, at: (f64, f64)) {
        let p = self.screen(at.0, at.1);
        self.send(
            Event::Mouse(mouse::Event::CursorMoved { position: p }),
            Some(at),
        );
    }

    fn escape(&mut self) -> bool {
        self.send(
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                modified_key: keyboard::Key::Named(keyboard::key::Named::Escape),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::default(),
                text: None,
                repeat: false,
            }),
            Some((0.0, 0.0)),
        )
    }

    fn home(&mut self) -> bool {
        self.send(
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Home),
                modified_key: keyboard::Key::Named(keyboard::key::Named::Home),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Home),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::default(),
                text: None,
                repeat: false,
            }),
            Some((0.0, 0.0)),
        )
    }

    /// A press-drag-release of the Select tool from `from` to `to`.
    fn drag(&mut self, from: (f64, f64), to: (f64, f64)) {
        self.press(from);
        self.moved(to);
        self.release(to);
    }
}

fn pin(number: &str, x: f64, y: f64, orientation: PinOrientation) -> signex_library::SymbolPin {
    let mut p = signex_library::SymbolPin::new(number, format!("PIN{number}"));
    p.position = [x, y];
    p.length = 2.54;
    p.orientation = orientation;
    p
}

fn rectangle(from: [f64; 2], to: [f64; 2]) -> SymbolGraphic {
    SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle { from, to },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    }
}

/// The canonical layout: a body rectangle with a pin whose tip sits
/// exactly on one of the rectangle's (undrawn) edge-midpoint handles.
fn pin_on_body_edge() -> Symbol {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Left));
    s
}

#[test]
fn dragging_a_pin_on_a_body_edge_moves_the_pin_not_the_rectangle() {
    let mut rig = Rig::new(pin_on_body_edge());
    let before = rig.editor.primitive().graphics[0].kind.clone();

    rig.drag((0.0, 0.0), (0.0, 2.54));

    assert_eq!(rig.editor.primitive().pins[0].position, [0.0, 2.54]);
    assert_eq!(
        rig.editor.primitive().graphics[0].kind,
        before,
        "an unselected rectangle must not resize from an invisible handle"
    );
}

#[test]
fn pressing_an_unselected_rectangle_corner_selects_it_without_resizing() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    let mut rig = Rig::new(s);

    // Just inside the corner: f32 screen round-trips make the exact
    // corner coordinate unreliable to hit from outside.
    rig.press((10.0, 5.0));
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Graphic(0))
    );
    rig.release((10.0, 5.0));

    // Now selected, the handle is drawn - and grabbable.
    rig.drag((10.0, 5.0), (12.7, 7.62));
    let SymbolGraphicKind::Rectangle { from, to } = rig.editor.primitive().graphics[0].kind.clone()
    else {
        panic!("rectangle expected");
    };
    assert_eq!(
        (from, to),
        ([0.0, -5.08], [12.7, 7.62]),
        "a selected rectangle resizes from its visible handle"
    );
}

#[test]
fn nearest_pin_wins_at_the_snap_pitch() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    s.pins.push(pin("2", 0.0, 1.27, PinOrientation::Right));
    s.pins.push(pin("3", 0.0, 2.54, PinOrientation::Right));
    let mut rig = Rig::new(s);

    rig.press((0.0, 2.54));
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Pin(2)),
        "pressing PIN3's tip must pick PIN3, not the first pin in range"
    );
}

#[test]
fn a_pin_can_be_dragged_by_its_body_line() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    // 4x zoom: the screen-space tip zone shrinks to ~0.67 mm, so a press
    // 1.9 mm along the 2.54 mm line can only be a hit through the
    // tip-to-base segment, never the tip disc.
    rig.editor.camera.scale *= 4.0;
    rig.editor.camera.offset = Point::new(450.0, 300.0);
    assert!(
        rig.editor.camera.scale > 8.0 / 0.67,
        "premise: tolerance well under 1.9 mm"
    );

    rig.drag((1.9, 0.0), (1.9, 2.54));

    assert_eq!(rig.editor.selected, Some(state::SymbolSelection::Pin(0)));
    assert_eq!(rig.editor.primitive().pins[0].position[1], 2.54);
}

#[test]
fn a_line_touching_a_pin_base_is_still_clickable_at_default_zoom() {
    // A clock-wedge style line starting at the pin's base (pin tip at
    // x = -2.54, base at the body edge x = 0). Pins used to outrank it
    // everywhere within 2.67 mm.
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Line {
            from: [0.0, -0.635],
            to: [0.9, 0.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    s.pins.push(pin("1", -2.54, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);

    rig.press((0.45, -0.32));

    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Graphic(1))
    );
}

#[test]
fn a_press_inside_the_body_near_a_pinned_edge_selects_the_body() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    s.pins.push(pin("1", -2.54, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);

    rig.drag((1.5, 1.0), (4.0, 1.0));

    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Graphic(0))
    );
    assert_eq!(
        rig.editor.primitive().pins[0].position,
        [-2.54, 0.0],
        "dragging the body must not move the pin beside it"
    );
}

#[test]
fn a_pin_still_wins_over_the_body_edge_it_is_attached_to() {
    // The pin's own line, outside the body, stays grabbable.
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    s.pins.push(pin("1", -2.54, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);

    rig.press((-1.27, 0.0));

    assert_eq!(rig.editor.selected, Some(state::SymbolSelection::Pin(0)));
}

#[test]
fn a_pin_on_a_selected_rectangles_handle_is_still_dragged_not_the_handle() {
    let mut rig = Rig::new(pin_on_body_edge());
    // The rectangle is selected first, so its edge-midpoint handle (0, 0)
    // is drawn - and the pin tip sits exactly on it.
    rig.editor.selected = Some(state::SymbolSelection::Graphic(0));
    let before = rig.editor.primitive().graphics[0].kind.clone();

    rig.drag((0.0, 0.0), (0.0, 2.54));

    assert_eq!(rig.editor.primitive().pins[0].position, [0.0, 2.54]);
    assert_eq!(rig.editor.primitive().graphics[0].kind, before);
}

#[test]
fn the_hover_cursor_agrees_with_the_press_over_a_pin_on_a_handle() {
    let mut rig = Rig::new(pin_on_body_edge());
    rig.editor.selected = Some(state::SymbolSelection::Graphic(0));
    let editor = &rig.editor;
    let canvas = SymbolCanvas::new(
        editor.primitive(),
        editor.selected.clone(),
        editor.tool,
        &editor.polygon_vertices,
        editor.active_part,
        false,
        &editor.camera,
        1.27,
        true,
        crate::render_config::GridStyle::default(),
        true,
        Color::WHITE,
        Color::from_rgb(0.2, 0.4, 0.9),
        Color::BLACK,
        Color::BLACK,
        Color::BLACK,
    );
    let cursor = mouse::Cursor::Available(rig.screen(0.0, 0.0));

    let shown = canvas::Program::mouse_interaction(&canvas, &rig.cstate, BOUNDS, cursor);

    assert_eq!(
        shown,
        mouse::Interaction::Crosshair,
        "a press here grabs the pin, so no resize cursor"
    );
}

#[test]
fn a_resize_handle_of_a_filtered_out_graphic_is_not_grabbable() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, 0.0], [5.08, 5.08]));
    let mut rig = Rig::new(s);
    rig.editor.selected = Some(state::SymbolSelection::Graphic(0));
    rig.editor.selection_filter.drawings = false;
    let before = rig.editor.primitive().graphics[0].kind.clone();

    rig.drag((5.08, 5.08), (8.0, 8.0));

    assert_eq!(rig.editor.primitive().graphics[0].kind, before);
}

#[test]
fn a_box_over_everything_stays_multiple_under_a_restricted_filter() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, 0.0], [5.08, 5.08]));
    s.pins.push(pin("1", -10.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    // Texts are off but the symbol has none, so the box still picks every
    // item - which must not become `All` while a pill is off.
    rig.editor.selection_filter.texts = false;

    rig.press((-12.0, -2.0));
    rig.moved((8.0, 8.0));
    rig.release((8.0, 8.0));

    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Multiple {
            pin_indices: vec![0],
            graphic_indices: vec![0],
        })
    );
}

#[test]
fn a_pin_drag_is_one_undo_step_and_marks_the_document_dirty() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    assert!(!rig.editor.dirty && rig.editor.undo_snapshots.is_empty());

    rig.press((0.0, 0.0));
    rig.moved((0.0, 1.27));
    rig.moved((0.0, 2.54));
    rig.release((0.0, 2.54));

    assert_eq!(
        rig.editor.undo_snapshots.len(),
        1,
        "one drag, one undo step"
    );
    assert!(rig.editor.dirty);
}

#[test]
fn the_pin_grab_zone_is_screen_space_not_world_space() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    rig.editor.camera.scale *= 4.0;
    rig.editor.camera.offset = Point::new(450.0, 300.0);

    // 8 px at 4x zoom is well under 1.5 mm: 1.4 mm behind the tip is
    // a miss now (it was a hit under the fixed world radius).
    rig.press((-1.4, 0.0));
    assert_eq!(rig.editor.selected, None);
}

#[test]
fn the_pins_filter_pill_gates_pin_picking() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    rig.editor.selection_filter.pins = false;

    rig.press((0.0, 0.0));
    assert_eq!(rig.editor.selected, None, "filtered-out pins are inert");

    rig.editor.selection_filter.pins = true;
    rig.release((0.0, 0.0));
    rig.press((0.0, 0.0));
    assert_eq!(rig.editor.selected, Some(state::SymbolSelection::Pin(0)));
}

#[test]
fn the_drawings_filter_pill_gates_graphic_picking_and_box_select() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, 0.0], [5.08, 5.08]));
    s.pins.push(pin("1", -10.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    rig.editor.selection_filter.drawings = false;

    rig.press((2.54, 2.54));
    assert_eq!(rig.editor.selected, None);
    rig.release((2.54, 2.54));

    // A window box over everything picks only what the filter allows.
    rig.press((-12.0, -2.0));
    rig.moved((8.0, 8.0));
    rig.release((8.0, 8.0));
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Multiple {
            pin_indices: vec![0],
            graphic_indices: vec![],
        })
    );
}

#[test]
fn escape_with_nothing_in_flight_is_left_to_the_app_ladder() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    rig.editor.tool = SymbolTool::AddPin;

    let claimed = rig.escape();

    assert!(
        !claimed,
        "an unclaimed Esc lets modals and the context menu take it first"
    );
    assert_eq!(rig.editor.tool, SymbolTool::AddPin);
}

#[test]
fn escape_cancels_an_in_flight_rectangle_and_keeps_the_tool() {
    let mut rig = Rig::new(Symbol::empty("T"));
    rig.editor.tool = SymbolTool::PlaceRectangle;

    rig.press((0.0, 0.0));
    assert!(rig.escape(), "an in-flight gesture claims the key");
    assert_eq!(rig.editor.tool, SymbolTool::PlaceRectangle);
    assert!(rig.cstate.rect_from.is_none());

    assert!(!rig.escape(), "the second Esc is the ladder's");
}

#[test]
fn escape_drops_an_armed_add_pin_press_so_release_places_nothing() {
    let mut rig = Rig::new(Symbol::empty("T"));
    rig.editor.tool = SymbolTool::AddPin;

    rig.press((0.0, 0.0));
    assert!(rig.escape());
    rig.release((0.0, 0.0));

    assert!(rig.editor.primitive().pins.is_empty());
}

#[test]
fn add_pin_lands_where_the_button_is_released() {
    let mut rig = Rig::new(Symbol::empty("T"));
    rig.editor.tool = SymbolTool::AddPin;

    rig.press((0.0, 0.0));
    assert!(
        rig.editor.primitive().pins.is_empty(),
        "pressing must not place the pin yet"
    );
    rig.moved((5.08, 2.54));
    rig.release((5.08, 2.54));

    let pins = &rig.editor.primitive().pins;
    assert_eq!(pins.len(), 1, "one gesture places exactly one pin");
    assert_eq!(pins[0].position, [5.08, 2.54]);
    assert_eq!(
        rig.editor.undo_snapshots.len(),
        1,
        "one placement, one undo"
    );
    assert!(rig.editor.dirty);
}

#[test]
fn fit_uses_the_real_canvas_size_not_a_hardcoded_viewport() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([-5.08, -2.54], [5.08, 2.54]));
    let mut rig = Rig::new(s);

    // The real path: the Home key publishes `Fit`, which only queues.
    rig.home();
    assert!(
        rig.editor.pending_view.is_some(),
        "Home must queue a fit, not apply a guessed viewport"
    );
    // Any event tick lets the canvas report its measured size.
    rig.send(
        Event::Window(iced::window::Event::RedrawRequested(
            std::time::Instant::now(),
        )),
        None,
    );

    assert_eq!(rig.editor.pending_view, None);
    let centre = rig.screen(0.0, 0.0);
    assert!(
        (centre.x - BOUNDS.width / 2.0).abs() < 1.0 && (centre.y - BOUNDS.height / 2.0).abs() < 1.0,
        "symbol centre {centre:?} should sit at the middle of the {}x{} canvas",
        BOUNDS.width,
        BOUNDS.height
    );
}

#[test]
fn a_pending_fit_does_not_swallow_the_event_that_triggers_the_measurement() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);

    rig.press((0.0, 0.0));
    rig.moved((0.0, 2.54));
    assert!(rig.editor.mid_drag, "the drag is under way");
    rig.home();
    assert!(rig.editor.pending_view.is_some());

    // The release arrives while the fit is still queued: it must both
    // measure/apply the view and end the drag.
    rig.release((0.0, 2.54));

    assert_eq!(rig.editor.pending_view, None, "the view was applied");
    assert!(
        !rig.editor.mid_drag,
        "the release still committed the drag gesture"
    );
}
