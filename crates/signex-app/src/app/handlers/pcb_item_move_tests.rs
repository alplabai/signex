//! #2870 — every selectable board object other than a footprint or via
//! moves by press-drag-release, driven from real pointer events on the
//! board canvas into `Signex::update`: tracks, arcs, fills, solid regions,
//! pours, keepouts, strings, the five drawings, dimensions and rooms. Each
//! drag is one undo step, snapped as a Move.

use std::path::{Path, PathBuf};

use iced::widget::canvas;
use iced::{Event, Rectangle, mouse};
use signex_engine::pcb::BoardItemId;
use signex_engine::pcb::item_move::moved_item;
use signex_types::pcb::{PcbBoard, Point, Room, RoomLayer, RoomOutline, RoomSource};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::pcb_canvas::PcbCanvasState;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1600.0,
    height: 1200.0,
};
const ROOM: Uuid = Uuid::from_u128(0x90);
const U1: Uuid = Uuid::from_u128(0x70);
const U2: Uuid = Uuid::from_u128(0x73);
const VIA: Uuid = Uuid::from_u128(0x74);
const TRACK_TO_VIA: Uuid = Uuid::from_u128(0x75);
/// The sheet the room is for; U2 is placed from it.
const SHEET: u128 = 0x5EE7;
const DELTA: Point = Point { x: 5.0, y: 0.0 };

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pt(x: f64, y: f64) -> serde_json::Value {
    serde_json::json!({"x": x, "y": y})
}

fn zone(n: u128, fill: &str, outline: &[(f64, f64)]) -> serde_json::Value {
    let outline: Vec<_> = outline.iter().map(|&(x, y)| pt(x, y)).collect();
    serde_json::json!({"uuid": id(n), "layer": "TopCopper", "fill_type": fill,
                       "outline": outline})
}

fn graphic(n: u128, kind: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut g = serde_json::json!({"uuid": id(n), "graphic_type": kind,
                                   "layer": "TopSilkscreen", "width": 0.2});
    let (serde_json::Value::Object(g_map), serde_json::Value::Object(extra)) = (&mut g, extra)
    else {
        unreachable!("objects");
    };
    g_map.extend(extra);
    g
}

/// One object of each kind, apart from each other, on a 0.5 mm grid.
fn board() -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "setup": {"grid_size": 0.5},
        "segments": [{"uuid": id(0x10), "start": pt(0.0, 0.0), "end": pt(4.0, 0.0),
                      "width": 0.25, "layer": "TopCopper", "net": 0},
                     // A track over the pour, for the press precedence.
                     {"uuid": id(0x11), "start": pt(21.0, 44.0), "end": pt(25.0, 44.0),
                      "width": 0.25, "layer": "TopCopper", "net": 0},
                     // From U1's pad to the via, for a drag of both.
                     {"uuid": TRACK_TO_VIA, "start": pt(28.0, 48.0), "end": pt(34.0, 48.0),
                      "width": 0.25, "layer": "TopCopper", "net": 1}],
        "nets": [{"number": 1, "name": "N"}],
        "vias": [{"uuid": VIA, "position": pt(34.0, 48.0), "diameter": 0.8, "drill": 0.4,
                  "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "free": {
            "arcs": [{"uuid": id(0x20), "start": pt(10.0, 0.0), "mid": pt(12.0, -2.0),
                      "end": pt(14.0, 0.0), "width": 0.25, "layer": "TopCopper", "net": 0}],
            "dimensions": [{"uuid": id(0x30), "start": pt(30.0, 0.0), "end": pt(36.0, 0.0),
                            "offset": 0.0, "layer": "TopSilkscreen", "line_width": 0.1,
                            "text_height": 1.0, "precision": 2}]
        },
        "texts": [{"uuid": id(0x40), "text": "REV", "position": pt(20.0, 10.0),
                   "layer": "TopSilkscreen", "font_size": 1.0, "rotation": 0.0}],
        "graphics": [
            graphic(0x50, "line", serde_json::json!({"start": pt(1.0, 30.0), "end": pt(5.0, 30.0)})),
            graphic(0x51, "circle", serde_json::json!({"center": pt(10.0, 30.0),
                                                      "end": pt(12.0, 30.0), "radius": 2.0})),
            graphic(0x52, "rect", serde_json::json!({"start": pt(20.0, 28.0), "end": pt(24.0, 32.0)})),
            graphic(0x53, "text_frame", serde_json::json!({"start": pt(30.0, 28.0),
                                                          "end": pt(36.0, 32.0), "text": "T",
                                                          "font_size": 1.0})),
            graphic(0x54, "arc", serde_json::json!({"center": pt(40.0, 30.0),
                                                   "start": pt(42.0, 30.0),
                                                   "end": pt(40.0, 32.0), "radius": 2.0}))
        ],
        "zones": [
            zone(0x60, "solid", &[(0.0, 40.0), (4.0, 40.0), (4.0, 44.0), (0.0, 44.0)]),
            zone(0x61, "solid", &[(10.0, 40.0), (14.0, 40.0), (12.0, 44.0)]),
            zone(0x62, "pour", &[(20.0, 40.0), (26.0, 40.0), (26.0, 46.0), (20.0, 46.0)]),
            zone(0x63, "keepout", &[(30.0, 40.0), (34.0, 40.0), (34.0, 44.0), (30.0, 44.0)])
        ],
        // A part beside the pour, for a drag of a mixed selection.
        "footprints": [{
            "uuid": U1, "reference": "U1", "position": pt(28.0, 48.0),
            "rotation": 0.0, "layer": "TopCopper", "locked": false,
            "pads": [{"uuid": id(0x71), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": pt(0.0, 0.0),
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"], "net": {"number": 1, "name": "N"}}]
        }, {
            // A part of the room's sheet, inside the room.
            "uuid": U2, "reference": "U2", "position": pt(42.0, 38.0),
            "rotation": 0.0, "layer": "TopCopper", "locked": false,
            "schematic_link": {"sheet": [id(SHEET)], "units": [id(0x7A)]},
            "pads": [{"uuid": id(0x72), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": pt(0.0, 0.0),
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"]}]
        }]
    }))
    .unwrap();
    // Inside the copper extent Fit frames: Fit reads no room, so a room
    // past the copper would sit off the test canvas.
    board.rooms.push(Room {
        uuid: ROOM,
        name: "R1".into(),
        source: Some(RoomSource {
            sheet: InstancePath::from_chain(vec![id(SHEET)]),
            channel: None,
        }),
        layer: RoomLayer::Top,
        outline: RoomOutline::Rect {
            x_mm: 38.0,
            y_mm: 34.0,
            w_mm: 5.0,
            h_mm: 5.0,
        },
        colour: "#336699".into(),
        keep_parts_inside: false,
        margin_mm: 0.0,
        locked: false,
    });
    board
}

struct Rig {
    app: Signex,
    path: PathBuf,
    state: PcbCanvasState,
    _dir: tempfile::TempDir,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    app.ui_state.pcb_snap.objects.clear();
    app.ui_state.pcb_snap.to_guides = false;
    app.ui_state.pcb_snap.to_axes = false;
    app.refresh_pcb_highlight();
    let mut state = PcbCanvasState::default();
    // The first event settles the view's fit, so pixels read after it hold.
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
    Rig {
        app,
        path,
        state,
        _dir: dir,
    }
}

impl Rig {
    fn pixel(&self, at: Point) -> iced::Point {
        self.app
            .interaction_state
            .pcb_canvas
            .camera_ref()
            .world_to_screen(iced::Point::new(at.x as f32, at.y as f32))
    }

    fn send(&mut self, event: Event, pixel: iced::Point) {
        let cursor = mouse::Cursor::Available(pixel);
        let action = canvas::Program::update(
            &self.app.interaction_state.pcb_canvas,
            &mut self.state,
            &event,
            BOUNDS,
            cursor,
        );
        if let Some(message) = action.and_then(|action| action.into_inner().0) {
            let _ = self.app.update(message);
        }
    }

    /// Press at `from`, move to `to` past the drag threshold, release.
    fn drag(&mut self, from: Point, to: Point) {
        let (a, b) = (self.pixel(from), self.pixel(to));
        self.send(Event::Mouse(mouse::Event::CursorMoved { position: a }), a);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        self.send(Event::Mouse(press), a);
        self.send(Event::Mouse(mouse::Event::CursorMoved { position: b }), b);
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        self.send(Event::Mouse(release), b);
    }

    fn board(&self) -> &PcbBoard {
        self.app
            .document_state
            .pcb_board(&self.path)
            .expect("board")
    }
}

fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

/// Each kind, pressed on its stroke or edge (a text frame inside its box),
/// drags by the pointer's offset as one undo step.
#[test]
fn every_board_object_kind_moves_by_dragging_it() {
    use BoardItemId as I;
    let cases: [(&str, BoardItemId, Point); 13] = [
        ("track", I::Segment(id(0x10)), Point::new(2.0, 0.0)),
        ("arc", I::Arc(id(0x20)), Point::new(12.0, -2.0)),
        ("dimension", I::Dimension(id(0x30)), Point::new(33.0, 0.0)),
        ("string", I::Text(id(0x40)), Point::new(20.0, 10.0)),
        ("line", I::Graphic(id(0x50)), Point::new(3.0, 30.0)),
        ("circle", I::Graphic(id(0x51)), Point::new(12.0, 30.0)),
        ("rectangle", I::Graphic(id(0x52)), Point::new(22.0, 28.0)),
        ("text frame", I::Graphic(id(0x53)), Point::new(33.0, 30.0)),
        ("graphic arc", I::Graphic(id(0x54)), Point::new(42.0, 30.0)),
        ("fill", I::Zone(id(0x60)), Point::new(2.0, 40.0)),
        ("solid region", I::Zone(id(0x61)), Point::new(12.0, 40.0)),
        ("polygon pour", I::Zone(id(0x62)), Point::new(23.0, 40.0)),
        ("keepout", I::Zone(id(0x63)), Point::new(32.0, 40.0)),
    ];
    let mut r = rig();
    let before = live(&r.app, &r.path);
    let zero = Point::new(0.0, 0.0);
    for (kind, item, at) in cases {
        let expected = moved_item(r.board(), item, DELTA).expect("on the board");
        r.drag(at, Point::new(at.x + DELTA.x, at.y + DELTA.y));
        let moved = moved_item(r.board(), item, zero).expect("still on the board");
        assert_eq!(moved, expected, "{kind} moved by the drag");
        let _ = r.app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(live(&r.app, &r.path), before, "{kind}: one undo step");
    }
}

#[test]
fn a_room_moves_by_dragging_its_edge() {
    let mut r = rig();
    let before = live(&r.app, &r.path);
    r.drag(Point::new(40.0, 34.0), Point::new(45.0, 34.0));
    let room = r
        .board()
        .rooms
        .iter()
        .find(|room| room.uuid == ROOM)
        .unwrap();
    let b = room.outline.bounds().expect("a box");
    assert_eq!((b.x_mm, b.y_mm), (43.0, 34.0), "the room moved by the drag");
    let _ = r.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&r.app, &r.path), before, "one undo step");
}

fn moved_by_drag(r: &mut Rig, item: BoardItemId, from: Point) -> bool {
    let expected = moved_item(r.board(), item, DELTA).expect("on the board");
    r.drag(from, Point::new(from.x + DELTA.x, from.y + DELTA.y));
    moved_item(r.board(), item, Point::new(0.0, 0.0)) == Some(expected)
}

/// Caner, 2026-11-24: a press anywhere inside a pour, fill, solid region,
/// keepout or room drags it, as one undo step.
#[test]
fn a_press_inside_an_area_or_room_drags_it() {
    let cases = [
        ("fill", BoardItemId::Zone(id(0x60)), Point::new(2.0, 42.0)),
        (
            "solid region",
            BoardItemId::Zone(id(0x61)),
            Point::new(12.0, 41.0),
        ),
        (
            "polygon pour",
            BoardItemId::Zone(id(0x62)),
            Point::new(23.0, 42.0),
        ),
        (
            "keepout",
            BoardItemId::Zone(id(0x63)),
            Point::new(32.0, 42.0),
        ),
    ];
    let mut r = rig();
    let before = live(&r.app, &r.path);
    for (kind, item, inside) in cases {
        assert!(
            moved_by_drag(&mut r, item, inside),
            "{kind} dragged from inside"
        );
        let _ = r.app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(live(&r.app, &r.path), before, "{kind}: one undo step");
    }
    r.drag(Point::new(40.0, 37.0), Point::new(45.0, 37.0));
    let room = r.board().rooms.iter().find(|room| room.uuid == ROOM);
    let b = room.and_then(|room| room.outline.bounds()).expect("a box");
    assert_eq!(
        (b.x_mm, b.y_mm),
        (43.0, 34.0),
        "the room dragged from inside"
    );
}

/// An object on top of an area wins the press: the track over the pour
/// moves and the pour stays.
#[test]
fn a_press_on_a_track_over_a_pour_drags_the_track() {
    let mut r = rig();
    let pour = moved_item(r.board(), BoardItemId::Zone(id(0x62)), Point::new(0.0, 0.0));
    let track = BoardItemId::Segment(id(0x11));
    assert!(
        moved_by_drag(&mut r, track, Point::new(23.0, 44.0)),
        "the track moved"
    );
    let after = moved_item(r.board(), BoardItemId::Zone(id(0x62)), Point::new(0.0, 0.0));
    assert_eq!(after, pour, "the pour under it stays");
}

/// A press on empty board starts a box: nothing moves and the box selects
/// what it encloses.
#[test]
fn a_press_on_empty_board_starts_a_box() {
    let mut r = rig();
    let before = live(&r.app, &r.path);
    r.drag(Point::new(17.0, 7.0), Point::new(27.0, 13.0));
    assert_eq!(live(&r.app, &r.path), before, "nothing moved");
    assert!(r.app.interaction_state.pcb_carry.is_none(), "no drag");
    let selection = r.app.interaction_state.pcb_selection.selection();
    assert!(
        selection.contains(pcb_geom::ItemRef::Text(id(0x40))),
        "the box selected the string inside it"
    );
}

fn u1_position(r: &Rig) -> Point {
    let fp = r.board().footprints.iter().find(|fp| fp.uuid == U1);
    fp.expect("U1").position
}

/// Box-select U1 with the pour (and the track inside it), then drag from
/// `press`: everything selected moves by one offset as one undo step.
fn drag_mixed_selection_from(press: Point) {
    let mut r = rig();
    let before = live(&r.app, &r.path);
    r.drag(Point::new(19.0, 39.0), Point::new(29.5, 49.5));
    let selection = r.app.interaction_state.pcb_selection.selection().clone();
    assert!(selection.footprints().contains(&U1), "the box took U1");
    assert!(
        selection.items().any(|i| matches!(i,
            pcb_geom::ItemRef::Zone { zone, .. } if zone == id(0x62))),
        "and the pour"
    );
    let pour = BoardItemId::Zone(id(0x62));
    let track = BoardItemId::Segment(id(0x11));
    let (pour_to, track_to) = (
        moved_item(r.board(), pour, DELTA).unwrap(),
        moved_item(r.board(), track, DELTA).unwrap(),
    );
    let u1_to = Point::new(u1_position(&r).x + DELTA.x, u1_position(&r).y);
    r.drag(press, Point::new(press.x + DELTA.x, press.y + DELTA.y));
    let zero = Point::new(0.0, 0.0);
    assert_eq!(
        moved_item(r.board(), pour, zero),
        Some(pour_to),
        "the pour moved"
    );
    assert_eq!(
        moved_item(r.board(), track, zero),
        Some(track_to),
        "the track moved"
    );
    assert_eq!(u1_position(&r), u1_to, "U1 moved with them");
    let _ = r.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&r.app, &r.path), before, "one undo step");
}

/// #2870: a press on the pour of a mixed selection carries the part too.
#[test]
fn a_press_on_a_selected_pour_drags_the_whole_selection() {
    drag_mixed_selection_from(Point::new(23.0, 42.0));
}

/// #2870: a press on the part of a mixed selection carries the pour too.
#[test]
fn a_press_on_a_selected_part_drags_the_whole_selection() {
    drag_mixed_selection_from(Point::new(28.0, 48.0));
}

/// The grab list is built once per board, view and filter, not on every
/// paint: a second paint shares the first one's shapes.
#[test]
fn the_grab_list_is_reused_until_the_board_or_filter_changes() {
    let mut r = rig();
    let grab = |r: &mut Rig| {
        r.app
            .with_pcb_selection(|sel, board| sel.paint(board, None).1)
            .expect("a board")
    };
    let first = grab(&mut r);
    assert!(!first.is_empty());
    assert!(std::sync::Arc::ptr_eq(&first, &grab(&mut r)), "reused");
    let mut filter = pcb_geom::PickFilter::default();
    filter.toggle(pcb_geom::PickKind::Polygons);
    // The app's filter, which every selection access hands to the canvas
    // state (`with_pcb_selection`), as the Selection Filter panel sets it.
    r.app.ui_state.pcb_pick_filter = filter;
    let filtered = grab(&mut r);
    assert!(
        !std::sync::Arc::ptr_eq(&first, &filtered),
        "a filter change rebuilds"
    );
    assert!(filtered.len() < first.len(), "the pour's shapes are gone");
}

fn position_of(r: &Rig, part: Uuid) -> Point {
    let fp = r.board().footprints.iter().find(|fp| fp.uuid == part);
    fp.expect("the part").position
}

/// A room selected with a part of its own, dragged from inside the room:
/// the room and the part each move by the offset once (the room's move
/// leaves the part to the drag), as one undo step.
#[test]
fn a_selected_part_in_a_dragged_room_moves_once() {
    let mut r = rig();
    let before = live(&r.app, &r.path);
    // A box round the room takes it and U2.
    r.drag(Point::new(37.5, 33.5), Point::new(43.5, 39.5));
    let selection = r.app.interaction_state.pcb_selection.selection().clone();
    assert!(selection.footprints().contains(&U2), "the box took U2");
    assert!(
        selection.contains(pcb_geom::ItemRef::Room(ROOM)),
        "and the room"
    );
    let u2 = position_of(&r, U2);
    r.drag(Point::new(40.0, 37.0), Point::new(45.0, 37.0));
    let room = r.board().rooms.iter().find(|room| room.uuid == ROOM);
    let b = room.and_then(|room| room.outline.bounds()).expect("a box");
    assert_eq!((b.x_mm, b.y_mm), (43.0, 34.0), "the room moved");
    assert_eq!(
        position_of(&r, U2),
        Point::new(u2.x + DELTA.x, u2.y),
        "U2 moved by the offset once, not twice"
    );
    let _ = r.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&r.app, &r.path), before, "one undo step");
}

/// A part and a via selected (the track between them is not), dragged by
/// the part: the track's ends on the pad and on the via both follow, so it
/// moves whole by the offset, as one undo step.
#[test]
fn a_track_from_a_moved_pad_to_a_moved_via_moves_whole() {
    use crate::pcb_canvas::PcbCanvasMsg;
    let mut r = rig();
    let before = live(&r.app, &r.path);
    let click = |r: &mut Rig, at: Point, mode| {
        let _ = r.app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at,
            tolerance_mm: 0.05,
            mode,
        }));
    };
    click(&mut r, Point::new(28.0, 48.0), pcb_geom::ClickMode::Replace);
    click(&mut r, Point::new(34.0, 48.0), pcb_geom::ClickMode::Toggle);
    let selection = r.app.interaction_state.pcb_selection.selection().clone();
    assert!(selection.footprints().contains(&U1), "U1 is selected");
    assert!(
        selection.contains(pcb_geom::ItemRef::Via(VIA)),
        "and the via"
    );
    assert!(
        !selection.contains(pcb_geom::ItemRef::Track(TRACK_TO_VIA)),
        "but not the track"
    );
    let track = BoardItemId::Segment(TRACK_TO_VIA);
    let expected = moved_item(r.board(), track, DELTA).unwrap();
    r.drag(Point::new(28.0, 48.0), Point::new(33.0, 48.0));
    let moved = moved_item(r.board(), track, Point::new(0.0, 0.0));
    assert_eq!(moved, Some(expected), "both ends moved");
    assert_eq!(position_of(&r, U1), Point::new(33.0, 48.0), "U1 moved");
    let via = r.board().vias.iter().find(|v| v.uuid == VIA).unwrap();
    assert_eq!(via.position, Point::new(39.0, 48.0), "the via moved");
    let _ = r.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&r.app, &r.path), before, "one undo step");
}
