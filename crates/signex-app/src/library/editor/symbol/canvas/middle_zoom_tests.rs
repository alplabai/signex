//! The middle-button drag zoom on the symbol canvas, through the real
//! chain: iced event -> `Program::update` -> `CanvasAction` ->
//! `SymbolEditorMsg` -> `apply_symbol_primitive_edit`.

use super::*;
use crate::app::SymbolDocument;
use crate::canvas::Camera;
use crate::library::editor::standalone::symbol::symbol_action_to_primitive_msg;
use crate::library::editor::symbol::updates::apply_symbol_primitive_edit;
use iced::Point;
use signex_library::SymbolFile;
use std::path::PathBuf;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 900.0,
    height: 600.0,
};
const PRESS: Point = Point::new(300.0, 200.0);

fn canvas_of(editor: &SymbolDocument) -> SymbolCanvas<'_> {
    SymbolCanvas::new(
        editor.primitive(),
        editor.selected.clone(),
        editor.tool,
        &editor.polygon_vertices,
        editor.view(),
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
    .with_zoom_step(editor.zoom_step)
}

struct Rig {
    editor: SymbolDocument,
    cstate: CanvasState,
    /// Every action the canvas published, as a debug string.
    published: Vec<String>,
}

impl Rig {
    fn new(tool: SymbolTool) -> Self {
        let mut editor = SymbolDocument::new(
            PathBuf::from("t.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("T")),
        );
        editor.pending_view = None;
        editor.camera = Camera::default();
        editor.tool = tool;
        Self {
            editor,
            cstate: CanvasState::default(),
            published: Vec::new(),
        }
    }

    fn send(&mut self, event: Event, at: Point) {
        let cursor = mouse::Cursor::Available(at);
        let canvas = canvas_of(&self.editor);
        let action = canvas::Program::update(&canvas, &mut self.cstate, &event, BOUNDS, cursor);
        if let Some((Some(published), _, _)) = action.map(canvas::Action::into_inner) {
            self.published.push(format!("{published:?}"));
            apply_symbol_primitive_edit(
                &mut self.editor,
                symbol_action_to_primitive_msg(published),
            );
        }
    }

    fn press(&mut self, button: mouse::Button, at: Point) {
        self.send(Event::Mouse(mouse::Event::ButtonPressed(button)), at);
    }

    fn release(&mut self, button: mouse::Button, at: Point) {
        self.send(Event::Mouse(mouse::Event::ButtonReleased(button)), at);
    }

    fn move_to(&mut self, at: Point) {
        let event = Event::Mouse(mouse::Event::CursorMoved { position: at });
        self.send(event, at);
    }

    fn redraw(&mut self, at: Point) {
        let now = std::time::Instant::now();
        let event = Event::Window(iced::window::Event::RedrawRequested(now));
        self.send(event, at);
    }

    fn scale(&self) -> f32 {
        self.editor.camera.scale
    }
}

#[test]
fn a_middle_drag_up_zooms_the_symbol_in() {
    let mut rig = Rig::new(SymbolTool::Select);
    let before = rig.scale();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 60.0));
    assert!(rig.scale() > before);
}

#[test]
fn a_middle_drag_down_zooms_the_symbol_out() {
    let mut rig = Rig::new(SymbolTool::Select);
    let before = rig.scale();
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y + 60.0));
    assert!(rig.scale() < before);
}

#[test]
fn a_sideways_middle_drag_leaves_the_symbol_view() {
    let mut rig = Rig::new(SymbolTool::Select);
    let (scale, offset) = (rig.scale(), rig.editor.camera.offset);
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x + 120.0, PRESS.y));
    assert_eq!((rig.scale(), rig.editor.camera.offset), (scale, offset));
}

#[test]
fn the_symbol_point_under_the_press_stays_put() {
    let mut rig = Rig::new(SymbolTool::Select);
    let pinned = rig.editor.camera.screen_to_world(PRESS, BOUNDS);
    rig.press(mouse::Button::Middle, PRESS);
    for (dx, dy) in [(40.0, -30.0), (-70.0, -90.0), (15.0, 25.0), (90.0, -140.0)] {
        rig.move_to(Point::new(PRESS.x + dx, PRESS.y + dy));
        let held = rig.editor.camera.screen_to_world(PRESS, BOUNDS);
        assert!(
            (held.x - pinned.x).abs() < 1e-2 && (held.y - pinned.y).abs() < 1e-2,
            "{pinned:?} moved to {held:?}"
        );
    }
}

#[test]
fn a_right_drag_still_pans_the_symbol() {
    let mut rig = Rig::new(SymbolTool::Select);
    let (scale, offset) = (rig.scale(), rig.editor.camera.offset);
    rig.press(mouse::Button::Right, PRESS);
    rig.move_to(Point::new(PRESS.x + 30.0, PRESS.y + 20.0));
    assert_eq!(rig.scale(), scale);
    assert_eq!(
        rig.editor.camera.offset,
        Point::new(offset.x + 30.0, offset.y + 20.0)
    );
    rig.release(mouse::Button::Right, Point::new(330.0, 220.0));
    assert!(
        !rig.published.iter().any(|p| p.contains("ShowContextMenu")),
        "{:?}",
        rig.published
    );
}

#[test]
fn a_right_click_still_opens_the_symbol_menu() {
    let mut rig = Rig::new(SymbolTool::Select);
    rig.press(mouse::Button::Right, PRESS);
    rig.release(mouse::Button::Right, PRESS);
    assert!(
        rig.published.iter().any(|p| p.contains("ShowContextMenu")),
        "{:?}",
        rig.published
    );
}

#[test]
fn a_middle_gesture_leaves_an_armed_symbol_tool_and_its_stash() {
    let mut rig = Rig::new(SymbolTool::PlacePolygon);
    rig.editor.polygon_vertices = vec![(0.0, 0.0), (5.0, 0.0)];
    rig.press(mouse::Button::Middle, PRESS);
    for dy in [-40.0, -80.0, -50.0] {
        let at = Point::new(PRESS.x + 10.0, PRESS.y + dy);
        rig.move_to(at);
        rig.redraw(at);
    }
    rig.release(mouse::Button::Middle, Point::new(310.0, 150.0));
    assert_eq!(rig.editor.tool, SymbolTool::PlacePolygon);
    assert_eq!(rig.editor.polygon_vertices, vec![(0.0, 0.0), (5.0, 0.0)]);
    for published in &rig.published {
        assert!(
            published.starts_with("ZoomFactor") || published.starts_with("CursorAt"),
            "the gesture published {published}"
        );
    }
}

#[test]
fn a_line_in_progress_follows_the_pointer_after_the_zoom() {
    let mut rig = Rig::new(SymbolTool::PlaceLine);
    rig.cstate.line_from = Some((0.0, 0.0));
    rig.press(mouse::Button::Middle, PRESS);
    let pointer = Point::new(PRESS.x + 40.0, PRESS.y - 70.0);
    rig.move_to(pointer);
    rig.redraw(pointer);
    let canvas = canvas_of(&rig.editor);
    let expected = world_for(&canvas, pointer.x, pointer.y, BOUNDS);
    assert_eq!(rig.cstate.line_cursor, Some(expected));
    assert_eq!(rig.cstate.line_from, Some((0.0, 0.0)));
}

#[test]
fn the_release_ends_the_symbol_zoom() {
    let mut rig = Rig::new(SymbolTool::Select);
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.release(mouse::Button::Middle, Point::new(PRESS.x, PRESS.y - 40.0));
    let after = rig.scale();
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.scale(), after);
}

#[test]
fn the_pointer_leaving_the_window_ends_the_symbol_zoom() {
    let mut rig = Rig::new(SymbolTool::Select);
    rig.press(mouse::Button::Middle, PRESS);
    rig.move_to(Point::new(PRESS.x, PRESS.y - 40.0));
    rig.send(Event::Mouse(mouse::Event::CursorLeft), PRESS);
    assert!(rig.cstate.middle_zoom.is_none());
    let after = rig.scale();
    rig.move_to(Point::new(PRESS.x, PRESS.y - 200.0));
    assert_eq!(rig.scale(), after);
}

#[test]
fn the_symbol_zoom_stops_at_its_limit() {
    let mut rig = Rig::new(SymbolTool::Select);
    rig.press(mouse::Button::Middle, PRESS);
    for step in 1..60 {
        rig.move_to(Point::new(PRESS.x, PRESS.y - 100.0 * step as f32));
    }
    assert_eq!(rig.scale(), Camera::MAX_SCALE);
}
