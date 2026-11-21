//! Caner, 2026-11-21: the board's Snap Options, from the real checkbox
//! message to a real canvas drag and tool click, on a 0.1 mm grid. Every
//! object sits off that grid, so landing on one can only be its snap.

use std::path::{Path, PathBuf};

use iced::widget::canvas;
use iced::{Event, Rectangle, mouse};
use serde_json::json;
use signex_engine::pcb::snap::SnapObject;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::command::CommandArgs;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::{PanelMsg, PcbBoardMsg};
use crate::pcb_canvas::{PcbCanvasState, SnapDistance, SnapScope};

const U1: Uuid = Uuid::from_u128(0x10);
const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1600.0,
    height: 1200.0,
};

/// U1 (carried) at (10, 10); U2's 2 x 1 mm pad at (30.03, 30.03); a pad on
/// the bottom only at (25.03, 10.03), a through pad at (25.03, 15.03); a
/// bare origin at (15.03, 40.03); a via, tracks, an arc, a zone, a text.
fn board(grid_mm: f64) -> PcbBoard {
    let pad = |id: u128, shape: &str, layers: serde_json::Value, kind: &str| {
        json!({
            "uuid": Uuid::from_u128(id), "number": "1", "pad_type": kind, "shape": shape,
            "position": {"x": 0.0, "y": 0.0}, "size": {"x": 2.0, "y": 1.0},
            "drill": if kind == "thru" { json!({"diameter": 0.5}) } else { json!(null) },
            "layers": layers, "net": {"number": 1, "name": "A"}
        })
    };
    let track = |id: u128, a: (f64, f64), b: (f64, f64)| {
        json!({
            "uuid": Uuid::from_u128(id), "start": {"x": a.0, "y": a.1},
            "end": {"x": b.0, "y": b.1}, "width": 0.25, "layer": "TopCopper", "net": 2
        })
    };
    let part = |id: u128, x: f64, y: f64, pads: Vec<serde_json::Value>| {
        json!({
            "uuid": Uuid::from_u128(id), "reference": format!("U{id}"), "layer": "TopCopper",
            "rotation": 0.0, "position": {"x": x, "y": y}, "pads": pads
        })
    };
    let mut board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": grid_mm, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "outline": [
            {"x": 0.03, "y": 0.03}, {"x": 70.03, "y": 0.03},
            {"x": 70.03, "y": 50.03}, {"x": 0.03, "y": 50.03}
        ],
        "footprints": [
            part(0x10, 10.0, 10.0, vec![pad(0x11, "rect", json!(["TopCopper"]), "smd")]),
            part(0x20, 30.03, 30.03, vec![pad(0x21, "rect", json!(["TopCopper"]), "smd")]),
            part(0x30, 25.03, 10.03, vec![pad(0x31, "rect", json!(["BottomCopper"]), "smd")]),
            part(0x40, 25.03, 15.03,
                 vec![pad(0x41, "rect", json!(["TopCopper", "BottomCopper"]), "thru")]),
            part(0x50, 15.03, 40.03, vec![])
        ],
        "vias": [{"uuid": Uuid::from_u128(0x60), "position": {"x": 20.03, "y": 20.03},
                  "diameter": 0.6, "drill": 0.3, "layers": ["TopCopper", "BottomCopper"]}],
        "segments": [
            track(0x70, (40.03, 5.03), (45.03, 5.03)),
            track(0x71, (60.03, 1.0), (60.03, 9.0)),
            track(0x72, (56.0, 5.03), (64.0, 5.03))
        ],
        "zones": [{"uuid": Uuid::from_u128(0x80), "layer": "TopCopper",
                   "outline": [{"x": 35.03, "y": 40.03}, {"x": 40.0, "y": 40.0},
                               {"x": 40.0, "y": 45.0}]}],
        "texts": [{"uuid": Uuid::from_u128(0x90), "text": "REV",
                   "position": {"x": 5.03, "y": 45.03}, "layer": "TopSilkscreen"}]
    }))
    .unwrap();
    board.free.arcs.push(signex_types::pcb_arc::TrackArc {
        uuid: Uuid::from_u128(0xA0),
        start: Point::new(50.03, 30.03),
        mid: Point::new(52.03, 32.03),
        end: Point::new(54.03, 30.03),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 2,
    });
    board
}

struct Rig {
    app: Signex,
    path: PathBuf,
    state: PcbCanvasState,
    _dir: tempfile::TempDir,
}

/// The board open with every object checkbox off, through the panel.
fn rig(grid_mm: f64) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board(grid_mm));
    app.refresh_pcb_highlight();
    let mut state = PcbCanvasState::default();
    // The first event settles the view's fit.
    let settled = canvas::Program::update(
        &app.interaction_state.pcb_canvas,
        &mut state,
        &Event::Mouse(mouse::Event::CursorLeft),
        BOUNDS,
        mouse::Cursor::Unavailable,
    );
    if let Some(message) = settled.and_then(|action| action.into_inner().0) {
        let _ = app.update(message);
    }
    let mut rig = Rig {
        app,
        path,
        state,
        _dir: dir,
    };
    for object in SnapObject::DEFAULT_ON {
        rig.toggle(object);
    }
    assert!(rig.app.ui_state.pcb_snap.objects.is_empty());
    rig
}

impl Rig {
    fn panel(&mut self, msg: PcbBoardMsg) {
        let msg = Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg)));
        let _ = self.app.update(msg);
    }

    fn toggle(&mut self, object: SnapObject) {
        self.panel(PcbBoardMsg::SnapObjectToggled(object));
    }

    /// Board mm that `px` screen pixels span at this zoom.
    fn mm(&self, px: f64) -> f64 {
        let canvas = &self.app.interaction_state.pcb_canvas;
        px / f64::from(canvas.camera_ref().scale())
    }

    fn pixel(&self, at: Point) -> iced::Point {
        self.app
            .interaction_state
            .pcb_canvas
            .camera_ref()
            .world_to_screen(iced::Point::new(at.x as f32, at.y as f32))
    }

    fn send(&mut self, event: Event, pixel: iced::Point) {
        let action = canvas::Program::update(
            &self.app.interaction_state.pcb_canvas,
            &mut self.state,
            &event,
            BOUNDS,
            mouse::Cursor::Available(pixel),
        );
        if let Some(message) = action.and_then(|action| action.into_inner().0) {
            let _ = self.app.update(message);
        }
    }

    fn move_to(&mut self, at: Point) {
        let pixel = self.pixel(at);
        let moved = mouse::Event::CursorMoved { position: pixel };
        self.send(Event::Mouse(moved), pixel);
    }

    fn press(&mut self, at: Point) {
        self.move_to(at);
        let pixel = self.pixel(at);
        let event = mouse::Event::ButtonPressed(mouse::Button::Left);
        self.send(Event::Mouse(event), pixel);
    }

    fn release(&mut self, at: Point) {
        let pixel = self.pixel(at);
        let event = mouse::Event::ButtonReleased(mouse::Button::Left);
        self.send(Event::Mouse(event), pixel);
    }

    fn click(&mut self, at: Point) {
        self.press(at);
        self.release(at);
    }

    fn board(&self) -> &PcbBoard {
        self.app
            .document_state
            .pcb_board(&self.path)
            .expect("board")
    }

    /// Drag U1 by its pad to `to`; where its origin landed. Undone after.
    fn drag_u1(&mut self, to: Point) -> Point {
        self.press(Point::new(10.0, 10.0));
        self.move_to(to);
        self.release(to);
        let at = self
            .board()
            .footprints
            .iter()
            .find(|f| f.uuid == U1)
            .unwrap()
            .position;
        let _ = self.app.update(Message::Edit(EditMsg::Undo));
        at
    }

    fn run(&mut self, id: &str) {
        let command = AppCommandId::new(id).expect("a catalog id");
        let _ = self
            .app
            .dispatch_command(&command, CommandArgs::none(), None);
    }

    /// Place a via with the tool at `at`; where it landed. Undone after.
    fn via_tool_at(&mut self, at: Point) -> Point {
        let before = self.board().vias.len();
        self.run("place_board_via");
        self.click(at);
        let placed = self.board().vias[before..].first().expect("a via").position;
        let _ = self.app.update(Message::Edit(EditMsg::Undo));
        self.app.clear_pcb_selection();
        placed
    }
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

fn on_grid(a: Point, step: f64) -> bool {
    let on = |v: f64| ((v / step).round() * step - v).abs() < 1e-6;
    on(a.x) && on(a.y)
}

/// Each object toggle, and a point only it pulls to.
const OBJECTS: [(SnapObject, (f64, f64)); 12] = [
    (SnapObject::TrackVertices, (40.03, 5.03)),
    (SnapObject::TrackLines, (42.53, 5.03)),
    (SnapObject::ArcCenters, (52.03, 30.03)),
    (SnapObject::Intersections, (60.03, 5.03)),
    (SnapObject::PadCenters, (30.03, 30.03)),
    (SnapObject::PadVertices, (31.03, 30.53)),
    (SnapObject::PadEdges, (30.33, 30.53)),
    (SnapObject::ViaCenters, (20.03, 20.03)),
    (SnapObject::Regions, (35.03, 40.03)),
    (SnapObject::BoardShape, (0.03, 50.03)),
    (SnapObject::FootprintOrigins, (15.03, 40.03)),
    (SnapObject::Texts, (5.03, 45.03)),
];

/// With every object off only the grid pulls; one toggle on makes that
/// object pull a drag and a tool click, and off stops it.
#[test]
fn each_object_toggle_pulls_only_while_it_is_on() {
    for (object, (x, y)) in OBJECTS {
        let mut rig = rig(0.1);
        let target = Point::new(x, y);
        let pointer = Point::new(x, y + rig.mm(4.0));

        let off = rig.drag_u1(pointer);
        assert!(
            !close(off, target) && on_grid(off, 0.1),
            "{object:?} off: {off:?}"
        );
        let off = rig.via_tool_at(pointer);
        assert!(
            !close(off, target) && on_grid(off, 0.1),
            "{object:?} tool: {off:?}"
        );

        rig.toggle(object);
        let on = rig.drag_u1(pointer);
        assert!(close(on, target), "{object:?} on: {on:?}");
        let on = rig.via_tool_at(pointer);
        assert!(close(on, target), "{object:?} tool on: {on:?}");

        rig.toggle(object);
        let again = rig.drag_u1(pointer);
        assert!(on_grid(again, 0.1), "{object:?} off again: {again:?}");
    }
}

#[test]
fn a_track_click_within_the_reach_of_a_pad_centre_lands_on_it() {
    let mut rig = rig(0.1);
    rig.toggle(SnapObject::PadCenters);
    let centre = Point::new(30.03, 30.03);
    rig.run("place_board_track");
    rig.click(Point::new(centre.x + rig.mm(5.0), centre.y));
    rig.click(Point::new(33.0, 33.0));
    let ends = rig
        .board()
        .segments
        .iter()
        .flat_map(|s| [s.start, s.end])
        .any(|p| close(p, centre));
    assert!(ends, "{:?}", rig.board().segments);
}

#[test]
fn beyond_the_reach_nothing_pulls() {
    let mut rig = rig(0.1);
    rig.toggle(SnapObject::PadCenters);
    let at = rig.drag_u1(Point::new(30.03, 30.03 + rig.mm(12.0)));
    assert!(on_grid(at, 0.1), "{at:?}");
}

#[test]
fn a_coarse_grid_keeps_its_larger_radius() {
    let mut rig = rig(2.54);
    rig.toggle(SnapObject::PadCenters);
    // Past 8 px, inside 0.5 x 2.54 mm.
    let gap = rig.mm(12.0).min(1.2).max(rig.mm(8.0) + 0.01);
    assert!(gap < 1.27, "this view keeps the test meaningful: {gap}");
    let target = Point::new(30.03, 30.03);
    let at = rig.drag_u1(Point::new(target.x, target.y + gap));
    assert!(close(at, target), "{at:?}");
    rig.panel(PcbBoardMsg::SnapDistancePicked(SnapDistance::Quarter));
    let quarter = rig.mm(8.0).max(0.25 * 2.54);
    if gap > quarter {
        let at = rig.drag_u1(Point::new(target.x, target.y + gap));
        assert!(!close(at, target), "a shorter Distance lets go: {at:?}");
    }
}

#[test]
fn current_layer_sees_a_through_pad_but_not_a_bottom_pad() {
    let mut rig = rig(0.1);
    rig.toggle(SnapObject::PadCenters);
    rig.panel(PcbBoardMsg::SnapScopePicked(SnapScope::CurrentLayer));
    let reach = rig.mm(4.0);
    let bottom = Point::new(25.03, 10.03);
    let at = rig.drag_u1(Point::new(bottom.x, bottom.y + reach));
    assert!(
        !close(at, bottom),
        "a bottom pad is off the top layer: {at:?}"
    );
    let through = Point::new(25.03, 15.03);
    let at = rig.drag_u1(Point::new(through.x, through.y + reach));
    assert!(close(at, through), "{at:?}");
    rig.panel(PcbBoardMsg::SnapScopePicked(SnapScope::AllLayers));
    let at = rig.drag_u1(Point::new(bottom.x, bottom.y + reach));
    assert!(close(at, bottom), "{at:?}");
}

/// Off stops object snapping only: the grid still applies.
#[test]
fn scope_off_stops_objects_and_keeps_the_grid() {
    let mut rig = rig(0.1);
    rig.toggle(SnapObject::PadCenters);
    rig.panel(PcbBoardMsg::SnapScopePicked(SnapScope::Off));
    let pad = Point::new(30.03, 30.03);
    let at = rig.drag_u1(Point::new(pad.x, pad.y + rig.mm(4.0)));
    assert!(!close(at, pad) && on_grid(at, 0.1), "{at:?}");
}

#[test]
fn grids_off_leaves_the_pointer_free() {
    let mut rig = rig(0.1);
    rig.panel(PcbBoardMsg::SnapToGridsToggled);
    let to = Point::new(33.333, 22.222);
    let at = rig.drag_u1(to);
    assert!(
        (at.x - to.x).abs() < 0.01 && (at.y - to.y).abs() < 0.01,
        "{at:?}"
    );
    assert!(!on_grid(at, 0.1), "{at:?}");
}

/// A tool honours guides as a drag does; turned off, it does not.
#[test]
fn a_tool_click_follows_the_guides_toggle() {
    use signex_types::pcb_options::{BoardGuide, GuideKind};
    let mut rig = rig(0.1);
    let guide = BoardGuide::new(
        Uuid::from_u128(0xB0),
        GuideKind::Vertical,
        Point::new(33.03, 0.0),
    );
    let path: &Path = &rig.path.clone();
    let mut options = rig.board().options.clone();
    options.guides.push(guide);
    let command = signex_engine::pcb::BoardCommand::SetBoardOptions {
        options: Box::new(options),
    };
    rig.app
        .execute_board_commands(path, vec![command], None)
        .expect("the guide");
    let pointer = Point::new(33.03 + rig.mm(4.0), 22.0);
    let at = rig.via_tool_at(pointer);
    assert!((at.x - 33.03).abs() < 1e-3, "{at:?}");
    rig.panel(PcbBoardMsg::SnapToGuidesToggled);
    let at = rig.via_tool_at(pointer);
    assert!(on_grid(at, 0.1), "{at:?}");
}

/// A press off the pad's centre still puts the centre on the target.
#[test]
fn a_part_pressed_off_centre_lands_its_pad_on_the_target() {
    let mut rig = rig(0.1);
    rig.toggle(SnapObject::PadCenters);
    let press = Point::new(10.3, 10.2);
    let target = Point::new(30.03, 30.03);
    let to = Point::new(
        target.x + press.x - 10.0,
        target.y + press.y - 10.0 + rig.mm(3.0),
    );
    rig.press(press);
    rig.move_to(to);
    rig.release(to);
    let at = rig
        .board()
        .footprints
        .iter()
        .find(|f| f.uuid == U1)
        .unwrap()
        .position;
    assert!(close(at, target), "{at:?}");
}
