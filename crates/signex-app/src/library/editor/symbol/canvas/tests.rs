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

    /// A right press-release at `at` with no motion between.
    fn right_click(&mut self, at: (f64, f64)) {
        self.send(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
            Some(at),
        );
        self.send(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)),
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
fn right_click_backs_out_of_a_placement_tool_in_the_same_two_steps_as_escape() {
    let mut rig = Rig::new(Symbol::empty("T"));
    rig.editor.tool = SymbolTool::PlaceRectangle;

    rig.press((0.0, 0.0));
    rig.right_click((5.0, 5.0));
    assert!(
        rig.cstate.rect_from.is_none(),
        "the first click cancels the gesture"
    );
    assert_eq!(rig.editor.tool, SymbolTool::PlaceRectangle);
    assert!(rig.editor.context_menu.is_none());

    rig.right_click((5.0, 5.0));
    assert_eq!(
        rig.editor.tool,
        SymbolTool::Select,
        "the next click leaves the tool, like the second Esc"
    );
    assert!(
        rig.editor.context_menu.is_none(),
        "leaving the tool opens no menu"
    );
}

#[test]
fn a_right_drag_in_a_placement_tool_pans_and_keeps_the_tool() {
    let mut rig = Rig::new(Symbol::empty("T"));
    rig.editor.tool = SymbolTool::AddPin;
    let before = rig.editor.camera.offset;

    rig.send(
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        Some((0.0, 0.0)),
    );
    rig.moved((20.0, 0.0));
    rig.send(
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)),
        Some((20.0, 0.0)),
    );

    assert_ne!(rig.editor.camera.offset, before, "the drag panned");
    assert_eq!(rig.editor.tool, SymbolTool::AddPin);
}

#[test]
fn a_right_click_in_the_select_tool_still_opens_the_context_menu() {
    let mut rig = Rig::new(Symbol::empty("T"));

    rig.right_click((0.0, 0.0));

    assert!(rig.editor.context_menu.is_some());
    assert_eq!(rig.editor.tool, SymbolTool::Select);
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

// ── Designator / value fields (#170) ─────────────────────────────

/// Body rectangle with an explicitly placed designator, so the field
/// tests do not depend on the default-placement rule.
fn body_with_designator_at(pos: [f64; 2]) -> Symbol {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    s.designator_pos = Some(pos);
    s
}

#[test]
fn pressing_the_designator_text_selects_the_reference_field() {
    let mut rig = Rig::new(body_with_designator_at([0.0, 7.62]));

    rig.press((0.5, 7.0));

    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Field(state::FieldKey::Reference))
    );
}

#[test]
fn an_untouched_symbol_has_pressable_fields_at_the_default_placement() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    let mut rig = Rig::new(s);

    // Designator above the top-left of the body, value below the
    // bottom-left (the text hangs down from its anchor).
    rig.press((0.5, 6.4));
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Field(state::FieldKey::Reference))
    );
    rig.release((0.5, 6.4));
    rig.press((0.5, -5.6));
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Field(state::FieldKey::Value))
    );
}

#[test]
fn dragging_the_designator_moves_it_by_the_snapped_delta_and_stores_the_position() {
    let mut rig = Rig::new(body_with_designator_at([0.0, 7.62]));
    assert_eq!(rig.editor.primitive().designator_pos, Some([0.0, 7.62]));

    rig.drag((0.5, 7.0), (3.04, 7.0));

    let pos = rig.editor.primitive().designator_pos.expect("stored");
    assert!(
        (pos[0] - 2.54).abs() < 1e-9 && (pos[1] - 7.62).abs() < 1e-9,
        "expected [2.54, 7.62], got {pos:?}"
    );
    assert!(rig.editor.dirty);
    assert_eq!(rig.editor.undo_snapshots.len(), 1, "one drag, one undo");
    assert_eq!(rig.editor.primitive().comment_pos, None, "value untouched");
}

#[test]
fn undo_restores_the_designator_position_before_the_drag() {
    let mut rig = Rig::new(body_with_designator_at([0.0, 7.62]));
    rig.drag((0.5, 7.0), (3.04, 7.0));
    assert_ne!(rig.editor.primitive().designator_pos, Some([0.0, 7.62]));

    apply_symbol_primitive_edit(
        &mut rig.editor,
        crate::library::messages::SymbolEditorMsg::Undo,
    );

    assert_eq!(rig.editor.primitive().designator_pos, Some([0.0, 7.62]));
}

#[test]
fn the_designators_filter_pill_gates_picking_the_designator() {
    let mut rig = Rig::new(body_with_designator_at([0.0, 7.62]));
    rig.editor.selection_filter.designators = false;

    rig.press((0.5, 7.0));
    assert_eq!(rig.editor.selected, None, "filtered-out fields are inert");
    rig.release((0.5, 7.0));

    rig.editor.selection_filter.designators = true;
    rig.press((0.5, 7.0));
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Field(state::FieldKey::Reference))
    );
}

#[test]
fn a_pin_under_the_designator_wins_the_press() {
    let mut s = body_with_designator_at([0.0, 7.62]);
    // Tip sits inside the designator's text box.
    s.pins.push(pin("1", 0.5, 7.0, PinOrientation::Left));
    let mut rig = Rig::new(s);

    rig.press((0.5, 7.0));

    assert_eq!(rig.editor.selected, Some(state::SymbolSelection::Pin(0)));
}

/// The renderer snapshot the canvas would draw for the rig's symbol.
fn snapshot_of(rig: &Rig) -> (SymbolCanvas<'_>, RendererSnapshot) {
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
    let snapshot = canvas.build_symbol_renderer_snapshot(&editor.selected, 1.0);
    (canvas, snapshot)
}

fn body_only() -> Symbol {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([0.0, -5.08], [10.16, 5.08]));
    s
}

#[test]
fn the_snapshot_draws_both_fields_with_their_text_and_default_positions() {
    let mut rig = Rig::new(body_only());
    let (canvas, snap) = snapshot_of(&rig);
    let texts = &snap.reference_value_texts;
    assert_eq!(texts.len(), 2, "designator + value are drawn");
    assert_eq!(texts[0].content, "U?");
    assert_eq!(texts[0].position, [0.0, 5.08 + 0.5 + 1.27]);
    assert_eq!(texts[1].content, "*");
    assert_eq!(texts[1].position, [0.0, -5.08 - 0.5]);
    assert_eq!(texts[0].color, to_rgba(canvas.text_color), "unselected");

    // An empty comment falls back to the placeholder.
    rig.editor.primitive_mut().comment.clear();
    let (_, snap) = snapshot_of(&rig);
    assert_eq!(snap.reference_value_texts[1].content, "Value");
}

#[test]
fn the_snapshot_highlights_a_selected_field_and_all() {
    let mut rig = Rig::new(body_only());
    rig.editor.selected = Some(state::SymbolSelection::Field(state::FieldKey::Reference));
    let (canvas, snap) = snapshot_of(&rig);
    let sel = to_rgba(canvas.selected_color);
    assert_eq!(snap.reference_value_texts[0].color, sel);
    assert_ne!(
        snap.reference_value_texts[1].color, sel,
        "value stays plain"
    );

    rig.editor.selected = Some(state::SymbolSelection::All);
    let (_, snap) = snapshot_of(&rig);
    assert!(snap.reference_value_texts.iter().all(|t| t.color == sel));
}

#[test]
fn a_click_that_does_not_move_a_default_field_changes_nothing() {
    let mut rig = Rig::new(body_only());

    // Press on the default designator, jitter within one snap step.
    rig.press((0.5, 6.4));
    rig.moved((0.3, 6.4));
    rig.release((0.3, 6.4));

    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Field(state::FieldKey::Reference))
    );
    assert_eq!(rig.editor.primitive().designator_pos, None);
    assert!(!rig.editor.dirty, "nothing moved, nothing to save");
    assert!(rig.editor.undo_snapshots.is_empty());
}

#[test]
fn a_designator_inside_the_body_wins_over_the_graphic_under_it() {
    let mut s = body_only();
    s.designator_pos = Some([1.0, 3.0]);
    let mut rig = Rig::new(s);

    rig.press((1.5, 2.4));

    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Field(state::FieldKey::Reference))
    );
}

/// Body with a pin, one stored designator and a default-placed value.
fn body_with_pin_and_fields() -> Symbol {
    let mut s = body_only();
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Left));
    s.designator_pos = Some([0.0, 7.62]);
    s
}

/// Drag the pin by (2.54, 0) and check both fields travelled with it.
fn assert_whole_body_drag_moves_both_fields(rig: &mut Rig) {
    let value_before = state::field_pos(rig.editor.primitive(), state::FieldKey::Value, 1);
    rig.drag((0.0, 0.0), (2.54, 0.0));
    let sym = rig.editor.primitive();
    assert_eq!(sym.pins[0].position, [2.54, 0.0], "the body moved");
    assert_eq!(sym.designator_pos, Some([2.54, 7.62]), "stored field");
    let value_after = state::field_pos(sym, state::FieldKey::Value, 1);
    assert_eq!(
        value_after,
        [value_before[0] + 2.54, value_before[1]],
        "default field"
    );
}

#[test]
fn an_all_selection_drag_moves_stored_and_default_fields_together() {
    let mut rig = Rig::new(body_with_pin_and_fields());
    rig.editor.selected = Some(state::SymbolSelection::All);
    assert_whole_body_drag_moves_both_fields(&mut rig);
}

#[test]
fn a_box_over_the_whole_body_moves_the_fields_with_it() {
    let mut rig = Rig::new(body_with_pin_and_fields());
    // With every pill on a full-body box collapses to `All`; a pill that
    // matches nothing here keeps it a `Multiple` covering the whole body.
    rig.editor.selection_filter.texts = false;
    rig.press((-6.0, -9.0));
    rig.moved((14.0, 6.0));
    rig.release((14.0, 6.0));
    assert!(matches!(
        rig.editor.selected,
        Some(state::SymbolSelection::Multiple { .. })
    ));
    assert_whole_body_drag_moves_both_fields(&mut rig);
}

#[test]
fn a_partial_selection_drag_leaves_the_fields_alone() {
    let mut rig = Rig::new(body_with_pin_and_fields());
    rig.drag((0.0, 0.0), (2.54, 0.0));
    assert_eq!(rig.editor.primitive().designator_pos, Some([0.0, 7.62]));
}

/// Two units: A (part 1) is a pin at the origin plus a rectangle, B
/// (part 2) the same at x = 40, and one stored designator shared by both.
fn two_unit_symbol_with_stored_designator() -> Symbol {
    let mut s = Symbol::empty("T");
    for (part, x) in [(1u8, 0.0), (2u8, 40.0)] {
        let mut p = pin("1", x, 0.0, PinOrientation::Left);
        p.part_number = part;
        s.pins.push(p);
        let mut g = rectangle([x, -5.08], [x + 10.16, 5.08]);
        g.part_number = part;
        s.graphics.push(g);
    }
    s.designator_pos = Some([0.0, 7.62]);
    s
}

#[test]
fn moving_one_whole_unit_leaves_the_shared_stored_fields_alone() {
    let mut rig = Rig::new(two_unit_symbol_with_stored_designator());
    rig.editor.active_part = 1;
    rig.press((-6.0, -9.0));
    rig.moved((14.0, 6.0));
    rig.release((14.0, 6.0));
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::All),
        "a box over all of unit A resolves to All"
    );

    rig.drag((0.0, 0.0), (2.54, 0.0));

    let sym = rig.editor.primitive();
    assert_eq!(sym.pins[0].position, [2.54, 0.0], "unit A moved");
    assert_eq!(sym.pins[1].position, [40.0, 0.0], "unit B stayed");
    assert_eq!(
        sym.designator_pos,
        Some([0.0, 7.62]),
        "the designator is shared with unit B, which did not move"
    );
}

#[test]
fn a_single_unit_symbol_whole_body_move_still_moves_stored_fields() {
    let mut s = Symbol::empty("T");
    let mut p = pin("1", 0.0, 0.0, PinOrientation::Left);
    p.part_number = 1;
    s.pins.push(p);
    let mut g = rectangle([0.0, -5.08], [10.16, 5.08]);
    g.part_number = 1;
    s.graphics.push(g);
    s.designator_pos = Some([0.0, 7.62]);
    let mut rig = Rig::new(s);
    rig.editor.active_part = 1;
    rig.editor.selected = Some(state::SymbolSelection::All);

    rig.drag((0.0, 0.0), (2.54, 0.0));

    let sym = rig.editor.primitive();
    assert_eq!(sym.pins[0].position, [2.54, 0.0]);
    assert_eq!(sym.designator_pos, Some([2.54, 7.62]));
}

#[test]
fn a_right_click_on_a_field_over_the_body_is_not_the_graphic() {
    let mut s = body_only();
    s.designator_pos = Some([1.0, 2.0]);
    let mut rig = Rig::new(s);

    rig.right_click((1.5, 1.4));

    let menu = rig.editor.context_menu.as_ref().expect("the menu opens");
    assert_eq!(
        menu.target,
        state::SymbolContextTarget::Field(state::FieldKey::Reference)
    );
    assert_eq!(
        rig.editor.selected,
        Some(state::SymbolSelection::Field(state::FieldKey::Reference)),
        "right-click selects the field first, like the left press"
    );
}

#[test]
fn a_right_click_on_a_pin_still_targets_the_pin() {
    let mut rig = Rig::new(pin_on_body_edge());

    rig.right_click((0.0, 0.0));

    let menu = rig.editor.context_menu.as_ref().expect("the menu opens");
    assert_eq!(menu.target, state::SymbolContextTarget::Pin(0));
}
