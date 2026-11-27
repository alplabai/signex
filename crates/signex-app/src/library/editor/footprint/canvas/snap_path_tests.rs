//! Footprint snap through the canvas input: each test presses, moves and
//! releases on the real handlers and reads the message they publish, or the
//! model after the real update applied it. Grid step 1 mm, Snap Distance
//! 0.203 mm (the defaults).

use iced::widget::canvas;
use iced::{Color, Point, Rectangle, mouse};
use signex_library::primitive::footprint::{FootprintFile, FpGraphic, FpGraphicKind};
use signex_library::{Footprint, RowId};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::{FootprintCanvas, FootprintCanvasState};
use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::{EditorMode, PadsTool, SketchTool};
use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
use crate::library::messages::{EditorMsg, FootprintEditorMsg as Msg, LibraryMessage};
use crate::library::state::EditorAddress;
use crate::panels::SnapOptionFlag;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};
const TOLERANCE: f64 = 1e-5;

struct Session {
    editor: FootprintDocument,
    cstate: FootprintCanvasState,
}

fn footprint_msg(action: Option<canvas::Action<LibraryMessage>>) -> Option<Msg> {
    match action?.into_inner().0? {
        LibraryMessage::EditorEvent {
            msg: EditorMsg::Footprint(msg),
            ..
        } => Some(msg),
        _ => None,
    }
}

fn canvas_of<'a>(editor: &'a FootprintDocument, cache: &'a canvas::Cache) -> FootprintCanvas<'a> {
    let primitive = editor.primitive();
    FootprintCanvas {
        state: &editor.state,
        address: EditorAddress::new(
            std::path::PathBuf::from("/tmp/lib.snxlib"),
            "footprints".to_string(),
            RowId::new(),
        ),
        bg_color: Color::WHITE,
        grid_color: Color::BLACK,
        cache,
        sketch: primitive.sketch.as_ref(),
        silk_f: &primitive.silk_f,
        silk_b: &primitive.silk_b,
        generated_silk: &primitive.generated.silk,
        zoom_step: 1.1,
        trackpad_pan: false,
        wheel_mode: Default::default(),
        mirror_card: None,
        body_3d: None,
    }
}

impl Session {
    fn new(footprint: Footprint) -> Self {
        Self {
            editor: FootprintDocument::new(
                std::path::PathBuf::from("snap-path.snxfpt"),
                FootprintFile::from_footprint(footprint),
            ),
            cstate: FootprintCanvasState::with_view(10.0, Point::new(100.0, 300.0)),
        }
    }

    fn cursor(&self, world: (f64, f64)) -> mouse::Cursor {
        mouse::Cursor::Available(self.cstate.world_to_screen(world))
    }

    fn press(&mut self, world: (f64, f64)) -> Option<Msg> {
        let cursor = self.cursor(world);
        let cache = canvas::Cache::new();
        let fpc = canvas_of(&self.editor, &cache);
        let action = fpc.on_button_pressed(&mut self.cstate, &mouse::Button::Left, BOUNDS, cursor);
        footprint_msg(action)
    }

    fn move_to(&mut self, world: (f64, f64)) -> Option<Msg> {
        let cursor = self.cursor(world);
        let cache = canvas::Cache::new();
        let fpc = canvas_of(&self.editor, &cache);
        footprint_msg(fpc.on_cursor_moved(&mut self.cstate, BOUNDS, cursor))
    }

    fn release(&mut self, world: (f64, f64)) -> Option<Msg> {
        let cursor = self.cursor(world);
        let cache = canvas::Cache::new();
        let fpc = canvas_of(&self.editor, &cache);
        let action = fpc.on_button_released(&mut self.cstate, &mouse::Button::Left, BOUNDS, cursor);
        footprint_msg(action)
    }

    fn click(&mut self, world: (f64, f64)) -> Option<Msg> {
        self.press(world);
        self.release(world)
    }

    fn apply(&mut self, msg: Msg) {
        apply_footprint_primitive_edit(&mut self.editor, msg);
    }

    /// A 1 mm square pad at `at`, added through the update.
    fn add_pad(&mut self, at: (f64, f64)) -> usize {
        self.apply(Msg::AddPad {
            x_mm: at.0,
            y_mm: at.1,
        });
        let idx = self.editor.state.pads.len() - 1;
        self.editor.state.pads[idx].size_mm = (1.0, 1.0);
        idx
    }

    /// Press at `from`, drag to `to`, apply the move and release.
    fn drag_pad(&mut self, from: (f64, f64), to: (f64, f64)) {
        let select = self.press(from).expect("the press selects the pad");
        self.apply(select);
        let moved = self.move_to(to).expect("the drag moves the pad");
        assert!(matches!(moved, Msg::MovePad { .. }), "got {moved:?}");
        self.apply(moved);
        self.release(to);
    }

    fn pad_at(&self, idx: usize) -> (f64, f64) {
        self.editor.state.pads[idx].position_mm
    }
}

fn assert_near(got: (f64, f64), want: (f64, f64)) {
    assert!(
        (got.0 - want.0).abs() < TOLERANCE && (got.1 - want.1).abs() < TOLERANCE,
        "got {got:?}, want {want:?}"
    );
}

/// A footprint whose sketch holds a Point at `a`, a Point at `b` and, with
/// `line`, the Line between them. Returns the two Point ids.
fn sketched(
    a: (f64, f64),
    b: (f64, f64),
    line: bool,
) -> (Footprint, SketchEntityId, SketchEntityId) {
    let plane = PlaneId::new();
    let (start, end) = (SketchEntityId::new(), SketchEntityId::new());
    let mut entities = vec![
        Entity::new(start, plane, EntityKind::Point { x: a.0, y: a.1 }),
        Entity::new(end, plane, EntityKind::Point { x: b.0, y: b.1 }),
    ];
    if line {
        entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start, end },
        ));
    }
    let mut footprint = Footprint::empty("snap");
    footprint.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities,
        ..SketchData::default()
    });
    (footprint, start, end)
}

fn sketch_session(footprint: Footprint, tool: SketchTool) -> Session {
    let mut session = Session::new(footprint);
    session.editor.state.mode = EditorMode::Sketch;
    session.editor.state.active_tool = tool;
    session
}

#[test]
fn dragging_a_pad_lands_its_centre_on_the_grid() {
    let mut s = Session::new(Footprint::empty("snap"));
    let pad = s.add_pad((2.0, 2.0));
    // Grabbed 0.3 / 0.2 off centre: the centre goes to (5.1, 3.9), node (5, 4).
    s.drag_pad((2.3, 2.2), (5.4, 4.1));
    assert_near(s.pad_at(pad), (5.0, 4.0));
}

#[test]
fn dragging_a_pad_snaps_to_another_pads_centre() {
    let mut s = Session::new(Footprint::empty("snap"));
    let pad = s.add_pad((2.0, 2.0));
    s.add_pad((5.5, 3.5));
    s.drag_pad((2.2, 2.1), (5.8, 3.7));
    assert_near(s.pad_at(pad), (5.5, 3.5));
}

#[test]
fn the_pad_centers_switch_turns_the_pad_hotspot_off() {
    let mut s = Session::new(Footprint::empty("snap"));
    let pad = s.add_pad((2.0, 2.0));
    s.add_pad((5.5, 3.5));
    s.apply(Msg::ActiveBarToggleSnap(SnapOptionFlag::PadCenters));
    s.drag_pad((2.2, 2.1), (5.8, 3.7));
    // No hotspot: the centre at (5.6, 3.6) rounds to the grid node.
    assert_near(s.pad_at(pad), (6.0, 4.0));
}

#[test]
fn a_track_starts_on_the_pad_centre_under_the_cursor() {
    let mut s = Session::new(Footprint::empty("snap"));
    s.add_pad((5.5, 3.5));
    s.editor.state.pads_tool = PadsTool::PlaceTrack;
    match s.click((5.6, 3.6)) {
        Some(Msg::TrackClick { x_mm, y_mm }) => assert_near((x_mm, y_mm), (5.5, 3.5)),
        other => panic!("expected a TrackClick, got {other:?}"),
    }
}

#[test]
fn placing_a_pad_takes_a_track_vertex_hotspot() {
    let mut s = Session::new(Footprint::empty("snap"));
    s.editor.primitive_mut().silk_f.push(FpGraphic {
        kind: FpGraphicKind::Line {
            from: [3.5, 1.5],
            to: [9.0, 1.5],
        },
        stroke_width: 0.15,
        filled: false,
        sketch_entity: None,
    });
    s.editor.state.pads_tool = PadsTool::PlacePad;
    match s.click((3.6, 1.6)) {
        Some(Msg::AddPad { x_mm, y_mm }) => assert_near((x_mm, y_mm), (3.5, 1.5)),
        other => panic!("expected an AddPad, got {other:?}"),
    }
}

#[test]
fn the_grids_switch_turns_grid_snap_off() {
    let mut s = Session::new(Footprint::empty("snap"));
    s.apply(Msg::ActiveBarToggleSnap(SnapOptionFlag::SnapToGrids));
    s.editor.state.pads_tool = PadsTool::PlacePad;
    match s.click((3.4, 2.6)) {
        Some(Msg::AddPad { x_mm, y_mm }) => assert_near((x_mm, y_mm), (3.4, 2.6)),
        other => panic!("expected an AddPad, got {other:?}"),
    }
}

#[test]
fn a_text_frame_commits_on_the_snapped_corners() {
    let mut s = Session::new(Footprint::empty("snap"));
    s.editor.state.pads_tool = PadsTool::PlaceTextFrame;
    s.press((1.2, 1.1));
    s.move_to((4.4, 3.3));
    match s.release((4.4, 3.3)) {
        Some(Msg::AddTextFrame {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
        }) => {
            assert_near((x_mm, y_mm), (1.0, 1.0));
            assert_near((w_mm, h_mm), (3.0, 2.0));
        }
        other => panic!("expected an AddTextFrame, got {other:?}"),
    }
}

#[test]
fn dragging_a_sketch_point_lands_it_on_the_grid() {
    let (footprint, point, _) = sketched((2.0, 2.0), (20.0, 20.0), false);
    let mut s = sketch_session(footprint, SketchTool::Select);
    s.press((2.3, 2.2));
    match s.move_to((5.4, 4.1)) {
        Some(Msg::SketchMovePoint { id, dx, dy }) => {
            assert_eq!(id, point);
            assert_near((dx, dy), (3.0, 2.0));
        }
        other => panic!("expected a SketchMovePoint, got {other:?}"),
    }
}

#[test]
fn a_dragged_point_does_not_snap_to_itself() {
    let (footprint, point, _) = sketched((2.0, 2.0), (20.0, 20.0), false);
    let mut s = sketch_session(footprint, SketchTool::Select);
    s.press((2.0, 2.0));
    // 6 px from the Point, inside its 12 px radius; the nearest node is (3, 2).
    match s.move_to((2.6, 2.0)) {
        Some(Msg::SketchMovePoint { id, dx, dy }) => {
            assert_eq!(id, point);
            assert_near((dx, dy), (1.0, 0.0));
        }
        other => panic!("expected a SketchMovePoint, got {other:?}"),
    }
}

#[test]
fn dragging_a_sketch_line_lands_it_on_the_grid() {
    let (footprint, ..) = sketched((0.0, 2.0), (10.0, 2.0), true);
    let mut s = sketch_session(footprint, SketchTool::Select);
    s.press((5.0, 2.3));
    match s.move_to((5.0, 4.4)) {
        Some(Msg::SketchMoveLine { dx, dy, .. }) => assert_near((dx, dy), (0.0, 2.0)),
        other => panic!("expected a SketchMoveLine, got {other:?}"),
    }
}

#[test]
fn drag_track_end_lands_the_endpoint_on_the_grid() {
    let (footprint, _, end) = sketched((0.0, 2.0), (10.0, 2.0), true);
    let mut s = sketch_session(footprint, SketchTool::DragTrackEnd);
    s.press((8.2, 2.2));
    match s.move_to((8.6, 4.1)) {
        Some(Msg::SketchMovePoint { id, dx, dy }) => {
            assert_eq!(id, end);
            assert_near((dx, dy), (0.0, 2.0));
        }
        other => panic!("expected a SketchMovePoint, got {other:?}"),
    }
}

#[test]
fn a_drawing_click_lands_on_the_grid_node_it_showed() {
    // The node (3, 0) is within 12 px of the Point at (2, 0); the cursor at
    // (3.4, 0) is not, so the snap shows the node and the click must land there.
    let (footprint, ..) = sketched((2.0, 0.0), (20.0, 20.0), false);
    let mut s = sketch_session(footprint, SketchTool::Line);
    match s.click((3.4, 0.0)) {
        Some(Msg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        }) => {
            assert_near((x_mm, y_mm), (3.0, 0.0));
            assert_eq!(snap_id, None, "the click must not reuse the nearby Point");
        }
        other => panic!("expected a SketchToolClick, got {other:?}"),
    }
}

#[test]
fn the_canvas_snaps_to_the_footprints_3d_body() {
    let mut s = Session::new(Footprint::empty("snap"));
    s.add_pad((0.0, 0.0));
    s.add_pad((6.0, 4.0));
    s.apply(Msg::ActiveBarToggleSnap(SnapOptionFlag::Body3dPoints));
    s.apply(Msg::ActiveBarToggleSnap(SnapOptionFlag::SnapToGrids));
    s.editor.state.pads_tool = PadsTool::PlacePad;
    let body = crate::library::editor::footprint::snap_objects::BodyBox::of(s.editor.primitive());
    let corner = body.corners()[1];
    let cursor = (corner.0 + 0.1, corner.1 + 0.05);
    let cache = canvas::Cache::new();
    let fpc = FootprintCanvas {
        body_3d: Some(body),
        ..canvas_of(&s.editor, &cache)
    };
    let world_cursor = mouse::Cursor::Available(s.cstate.world_to_screen(cursor));
    fpc.on_button_pressed(&mut s.cstate, &mouse::Button::Left, BOUNDS, world_cursor);
    let released =
        fpc.on_button_released(&mut s.cstate, &mouse::Button::Left, BOUNDS, world_cursor);
    match footprint_msg(released) {
        Some(Msg::AddPad { x_mm, y_mm }) => assert_near((x_mm, y_mm), corner),
        other => panic!("expected an AddPad on the body corner, got {other:?}"),
    }
}
