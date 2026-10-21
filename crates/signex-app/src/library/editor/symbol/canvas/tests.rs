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
    fn send(&mut self, event: Event, at: Option<(f64, f64)>) {
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
        if let Some((Some(published), _, _)) = action.map(canvas::Action::into_inner) {
            apply_symbol_primitive_edit(
                &mut self.editor,
                symbol_action_to_primitive_msg(published),
            );
        }
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

    fn escape(&mut self) {
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
        );
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

    // 1.9 mm along the 2.54 mm body: far outside the old 1.5 mm tip disc.
    rig.drag((1.9, 0.0), (1.9, 2.54));

    assert_eq!(rig.editor.selected, Some(state::SymbolSelection::Pin(0)));
    assert_eq!(rig.editor.primitive().pins[0].position[1], 2.54);
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
fn escape_returns_the_add_pin_tool_to_select() {
    let mut s = Symbol::empty("T");
    s.pins.push(pin("1", 0.0, 0.0, PinOrientation::Right));
    let mut rig = Rig::new(s);
    rig.editor.tool = SymbolTool::AddPin;

    rig.escape();

    assert_eq!(rig.editor.tool, SymbolTool::Select);
}

#[test]
fn escape_cancels_an_in_flight_rectangle_before_leaving_the_tool() {
    let mut rig = Rig::new(Symbol::empty("T"));
    rig.editor.tool = SymbolTool::PlaceRectangle;

    rig.press((0.0, 0.0));
    rig.escape();
    assert_eq!(rig.editor.tool, SymbolTool::PlaceRectangle);
    assert!(rig.cstate.rect_from.is_none());

    rig.escape();
    assert_eq!(rig.editor.tool, SymbolTool::Select);
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
}

#[test]
fn fit_uses_the_real_canvas_size_not_a_hardcoded_viewport() {
    let mut s = Symbol::empty("T");
    s.graphics.push(rectangle([-5.08, -2.54], [5.08, 2.54]));
    let mut rig = Rig::new(s);

    rig.editor.pending_view = Some(crate::app::SymbolViewRequest::Fit);
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
