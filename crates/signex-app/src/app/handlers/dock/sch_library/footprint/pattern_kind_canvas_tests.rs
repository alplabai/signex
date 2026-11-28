//! #3232: an armed centre pick takes the click under every tool, driven
//! as a real press and release through the footprint canvas
//! (`canvas::Program::update`), each message it publishes fed back to the
//! app before the next event. A click on a Point picks that Point.

use std::path::{Path, PathBuf};

use iced::event::Event;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, mouse};
use signex_bake::array::PatternKind;
use signex_sketch::array::{ArrayId, ArrayKind};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;

use super::tests::{app_on_a_linear, editor, is_idle, kind, send, sketch, sketch_mut};
use crate::app::Signex;
use crate::library::editor::footprint::canvas::{FootprintCanvas, FootprintCanvasState};
use crate::library::editor::footprint::{EditorMode, SketchTool};
use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

/// The app of [`app_on_a_linear`] in sketch mode under `tool`, with a drawn
/// Point at (3, 4).
fn sketching_with(path: &Path, tool: SketchTool) -> (Signex, ArrayId, SketchEntityId) {
    let (mut app, array_id) = app_on_a_linear(path);
    let drawn = SketchEntityId::new();
    let sketch = sketch_mut(&mut app, path);
    let plane = sketch.planes[0].id;
    sketch.entities.push(Entity::new(
        drawn,
        plane,
        EntityKind::Point { x: 3.0, y: 4.0 },
    ));
    let doc = app.document_state.footprint_editors.get_mut(path).unwrap();
    doc.state.mode = EditorMode::Sketch;
    doc.state.active_tool = tool;
    doc.state.fit_pending = false;
    (app, array_id, drawn)
}

/// Press and release the left button at `at_mm` on the canvas, feeding
/// each message the canvas publishes to the app before the next event.
/// Two cursor moves first let the camera settle (a resize or fit takes
/// the event it happens on).
fn click_canvas(app: &mut Signex, path: &Path, at_mm: (f64, f64)) {
    let mut cstate = FootprintCanvasState::with_view(20.0, Point::new(400.0, 300.0));
    // Mid-canvas, clear of the edge pan's margin.
    let middle = Point::new(400.0, 300.0);
    let warm_up = mouse::Event::CursorMoved { position: middle };
    deliver(app, path, &mut cstate, warm_up, middle);
    deliver(app, path, &mut cstate, warm_up, middle);
    let at = cstate.world_to_screen(at_mm);
    deliver(
        app,
        path,
        &mut cstate,
        mouse::Event::CursorMoved { position: at },
        at,
    );
    deliver(
        app,
        path,
        &mut cstate,
        mouse::Event::ButtonPressed(mouse::Button::Left),
        at,
    );
    deliver(
        app,
        path,
        &mut cstate,
        mouse::Event::ButtonReleased(mouse::Button::Left),
        at,
    );
}

/// One pointer event through the canvas of the app's editor, and the
/// message it publishes, if any, through the app.
fn deliver(
    app: &mut Signex,
    path: &Path,
    cstate: &mut FootprintCanvasState,
    event: mouse::Event,
    at: Point,
) {
    let published = {
        let doc = editor(app, path);
        let cache = canvas::Cache::new();
        let fpc = FootprintCanvas {
            state: &doc.state,
            address: EditorAddress::new(
                path.to_path_buf(),
                String::new(),
                signex_library::RowId::from_uuid(uuid::Uuid::nil()),
            ),
            bg_color: Color::WHITE,
            grid_color: Color::BLACK,
            cache: &cache,
            sketch: doc.primitive().sketch.as_ref(),
            silk_f: &[],
            silk_b: &[],
            generated_silk: &[],
            zoom_step: 1.1,
            trackpad_pan: false,
            wheel_mode: Default::default(),
            mirror_card: None,
            body_3d: None,
        };
        let cursor = mouse::Cursor::Available(at);
        canvas::Program::update(&fpc, cstate, &Event::Mouse(event), BOUNDS, cursor)
            .and_then(|action| action.into_inner().0)
    };
    match published {
        None => {}
        Some(LibraryMessage::EditorEvent {
            msg: EditorMsg::Footprint(msg),
            ..
        }) => send(app, path, msg),
        Some(other) => panic!("unexpected canvas message {other:?}"),
    }
}

/// Arm the switch to Polar, click the drawn Point under `tool`: the
/// pattern turns about that Point, and no Point is added.
fn the_pick_takes_a_click_on_a_point(tool: SketchTool, name: &str) {
    let path = PathBuf::from(format!("kind-pick-{name}.snxfpt"));
    let (mut app, array_id, drawn) = sketching_with(&path, tool);
    let entities = sketch(&app, &path).entities.len();
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);

    click_canvas(&mut app, &path, (3.0, 4.0));

    let ArrayKind::Polar { center, .. } = kind(&app, &path, array_id) else {
        panic!(
            "{name}: a polar pattern, not {:?}",
            kind(&app, &path, array_id)
        );
    };
    assert_eq!(*center, drawn, "{name}: the Point under the cursor");
    assert_eq!(
        sketch(&app, &path).entities.len(),
        entities,
        "{name}: the pick adds no Point"
    );
    assert!(is_idle(&app, &path), "{name}: the pick is answered");
    assert_eq!(editor(&app, &path).history.undo_len(), 1, "{name}");
}

#[test]
fn under_select_the_pick_takes_a_click_on_a_point() {
    the_pick_takes_a_click_on_a_point(SketchTool::Select, "select");
}

#[test]
fn under_point_the_pick_takes_a_click_on_a_point() {
    the_pick_takes_a_click_on_a_point(SketchTool::Point, "point");
}

#[test]
fn under_chamfer_the_pick_takes_a_click_on_a_point() {
    the_pick_takes_a_click_on_a_point(SketchTool::Chamfer, "chamfer");
}

/// An undo drops an armed pick: the next click is an ordinary Select
/// click, which adds no Point, records no step and leaves the document
/// as it was.
#[test]
fn an_undo_drops_an_armed_pick() {
    let path = PathBuf::from("kind-pick-undo.snxfpt");
    let (mut app, array_id, _) = sketching_with(&path, SketchTool::Select);
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Grid);
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    let doc = app.document_state.footprint_editors.get_mut(&path).unwrap();
    assert!(doc.undo());
    assert!(is_idle(&app, &path), "the undo drops the pick");
    let entities = sketch(&app, &path).entities.len();
    let dirty = editor(&app, &path).is_dirty();

    click_canvas(&mut app, &path, (5.0, 6.0));

    assert!(matches!(
        kind(&app, &path, array_id),
        ArrayKind::Linear { .. }
    ));
    assert_eq!(
        sketch(&app, &path).entities.len(),
        entities,
        "no stray Point"
    );
    assert_eq!(editor(&app, &path).is_dirty(), dirty);
    assert!(!dirty, "back at the saved state");
    assert_eq!(editor(&app, &path).history.undo_len(), 0, "no undo step");
}

/// A pick whose pattern is gone mints no centre and records no step.
#[test]
fn a_pick_for_a_pattern_that_is_gone_changes_nothing() {
    let path = PathBuf::from("kind-pick-gone.snxfpt");
    let (mut app, array_id, _) = sketching_with(&path, SketchTool::Select);
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    sketch_mut(&mut app, &path).arrays.clear();
    let entities = sketch(&app, &path).entities.len();

    click_canvas(&mut app, &path, (5.0, 6.0));

    assert!(is_idle(&app, &path));
    assert_eq!(
        sketch(&app, &path).entities.len(),
        entities,
        "no stray Point"
    );
    assert_eq!(editor(&app, &path).history.undo_len(), 0);
    assert!(!editor(&app, &path).is_dirty());
}
