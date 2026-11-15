//! #2396 — board selection through real canvas pointer events: the same
//! rules for one selected item or many, an empty click or box that clears,
//! and a board tool that gets every click while something is selected.

use std::collections::BTreeSet;
use std::path::PathBuf;

use iced::keyboard::{Event as KeyEvent, Modifiers};
use iced::widget::canvas::Program;
use iced::{Event, Rectangle, mouse};
use pcb_geom::ItemRef;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::canvas::Camera;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvasState;

const U1: Uuid = Uuid::from_u128(0x10);
const D1: Uuid = Uuid::from_u128(0x20);
const D2: Uuid = Uuid::from_u128(0x30);

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1200.0,
    height: 800.0,
};

/// Empty board space: no footprint, pad, track or via within reach, but on
/// the ratsnest line from U1's pad 1 to D1's pad 2.
const EMPTY_ON_RATSNEST: (f64, f64) = (20.0, 10.0);
/// Empty board space well away from everything.
const EMPTY: (f64, f64) = (30.0, 22.0);

/// U1, D1 and D2 along y = 10, at x = 10, 30 and 50, each with two 1 mm
/// pads 2 mm either side of its origin. U1's pad 1 and D1's pad 2 share
/// net 1, so a ratsnest line runs between them.
fn board() -> PcbBoard {
    let footprint = |id: Uuid, reference: &str, x: f64, pad: u128| {
        let net = |on: bool| {
            if on {
                serde_json::json!({"number": 1, "name": "N1"})
            } else {
                serde_json::Value::Null
            }
        };
        serde_json::json!({
            "uuid": id, "reference": reference,
            "position": {"x": x, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "pads": [
                {"uuid": Uuid::from_u128(pad), "number": "1", "pad_type": "smd",
                 "shape": "rect", "position": {"x": 2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
                 "net": net(id == U1)},
                {"uuid": Uuid::from_u128(pad + 1), "number": "2", "pad_type": "smd",
                 "shape": "rect", "position": {"x": -2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
                 "net": net(id == D1)}
            ]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3, "via_diameter": 0.8,
                  "via_drill": 0.4, "clearance": 0.2},
        "nets": [{"number": 1, "name": "N1"}],
        "footprints": [
            footprint(U1, "U1", 10.0, 0x11),
            footprint(D1, "D1", 30.0, 0x21),
            footprint(D2, "D2", 50.0, 0x31)
        ]
    }))
    .unwrap()
}

/// The app with the board open and the pointer's canvas state.
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
    let mut rig = Rig {
        app,
        path,
        state: PcbCanvasState::default(),
        _dir: dir,
    };
    // The first event sizes the canvas (and fits a pending board fit).
    rig.send(
        Event::Mouse(mouse::Event::CursorMoved {
            position: iced::Point::new(600.0, 400.0),
        }),
        iced::Point::new(600.0, 400.0),
    );
    // Centre on the parts with room around them, so every point a test
    // uses is on the canvas.
    let canvas = &rig.app.interaction_state.pcb_canvas;
    let step = Camera::zoom_step_factor(Camera::ZOOM_STEP_PERCENT_DEFAULT);
    for _ in 0..3 {
        let _ = canvas.zoom_about_centre(false, step);
    }
    canvas.center_on_world(30.0, 10.0);
    rig
}

impl Rig {
    /// Run `event` through the canvas at `cursor` and the message it
    /// publishes through `Signex::update`.
    fn send(&mut self, event: Event, cursor: iced::Point) {
        let canvas = &self.app.interaction_state.pcb_canvas;
        let action = canvas.update(
            &mut self.state,
            &event,
            BOUNDS,
            mouse::Cursor::Available(cursor),
        );
        if let Some(message) = action.and_then(|a| a.into_inner().0) {
            let _ = self.app.update(message);
        }
    }

    /// The canvas pixel showing board point `(x, y)`.
    fn screen(&self, (x, y): (f64, f64)) -> iced::Point {
        let canvas = &self.app.interaction_state.pcb_canvas;
        let world = |sx: f32, sy: f32| canvas.screen_to_world(iced::Point::new(sx, sy), BOUNDS);
        let (o, ex, ey) = (world(0.0, 0.0), world(1.0, 0.0), world(0.0, 1.0));
        let (ax, ay) = (ex.x - o.x, ex.y - o.y);
        let (bx, by) = (ey.x - o.x, ey.y - o.y);
        let (wx, wy) = (x - o.x, y - o.y);
        let det = ax * by - bx * ay;
        let point = iced::Point::new(
            ((wx * by - bx * wy) / det) as f32,
            ((ax * wy - wx * ay) / det) as f32,
        );
        assert!(
            BOUNDS.contains(point),
            "board point ({x}, {y}) is off the canvas at {point:?}"
        );
        point
    }

    fn mods(&mut self, mods: Modifiers) {
        let event = Event::Keyboard(KeyEvent::ModifiersChanged(mods));
        self.send(event, iced::Point::ORIGIN);
    }

    fn press(&mut self, at: iced::Point) {
        let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        self.send(event, at);
    }

    fn move_to(&mut self, at: iced::Point) {
        self.send(Event::Mouse(mouse::Event::CursorMoved { position: at }), at);
    }

    fn release(&mut self, at: iced::Point) {
        let event = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        self.send(event, at);
    }

    /// Press and release in place at board point `at`, with `mods` held.
    fn click_with(&mut self, mods: Modifiers, at: (f64, f64)) {
        let p = self.screen(at);
        self.mods(mods);
        self.move_to(p);
        self.press(p);
        self.release(p);
        self.mods(Modifiers::default());
    }

    fn click(&mut self, at: (f64, f64)) {
        self.click_with(Modifiers::default(), at);
    }

    /// Drag a box from board point `from` to `to`, with `mods` held.
    fn drag_box(&mut self, mods: Modifiers, from: (f64, f64), to: (f64, f64)) {
        let (from, to) = (self.screen(from), self.screen(to));
        self.mods(mods);
        self.press(from);
        self.move_to(iced::Point::new(
            (from.x + to.x) / 2.0,
            (from.y + to.y) / 2.0,
        ));
        self.move_to(to);
        self.release(to);
        self.mods(Modifiers::default());
    }

    /// The footprints selected, whole.
    fn selected(&self) -> BTreeSet<Uuid> {
        self.app
            .interaction_state
            .pcb_selection
            .selection()
            .items()
            .filter_map(|item| match item {
                ItemRef::Footprint(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    fn selection_len(&self) -> usize {
        self.app
            .interaction_state
            .pcb_selection
            .selection()
            .items()
            .count()
    }

    /// U1, D1 and D2 selected by a click and two Ctrl clicks.
    fn select_all_three(&mut self) {
        self.click((10.0, 10.0));
        self.click_with(Modifiers::CTRL, (30.0, 10.0));
        self.click_with(Modifiers::CTRL, (50.0, 10.0));
        assert_eq!(self.selected(), BTreeSet::from([U1, D1, D2]));
    }

    fn board(&self) -> &PcbBoard {
        self.app
            .document_state
            .pcb_board(&self.path)
            .expect("board")
    }
}

fn set(ids: &[Uuid]) -> BTreeSet<Uuid> {
    ids.iter().copied().collect()
}

#[test]
fn a_plain_click_on_empty_space_clears_one_or_many() {
    let mut rig = rig();
    rig.click((10.0, 10.0));
    assert_eq!(rig.selected(), set(&[U1]));
    rig.click(EMPTY);
    assert_eq!(rig.selection_len(), 0, "one selected item is cleared");

    rig.select_all_three();
    rig.click(EMPTY);
    assert_eq!(rig.selection_len(), 0, "three selected items are cleared");
}

#[test]
fn the_ratsnest_line_is_empty_space() {
    let mut rig = rig();
    rig.select_all_three();
    rig.click(EMPTY_ON_RATSNEST);
    assert_eq!(rig.selection_len(), 0);
}

#[test]
fn a_plain_click_on_an_unselected_item_selects_only_it() {
    let mut rig = rig();
    rig.click((10.0, 10.0));
    rig.click_with(Modifiers::CTRL, (30.0, 10.0));
    assert_eq!(rig.selected(), set(&[U1, D1]));
    rig.click((50.0, 10.0));
    assert_eq!(rig.selected(), set(&[D2]));
    assert_eq!(rig.selection_len(), 1);
}

#[test]
fn a_plain_click_on_a_selected_item_keeps_only_it() {
    let mut rig = rig();
    rig.select_all_three();
    rig.click((30.0, 10.0));
    assert_eq!(rig.selected(), set(&[D1]));
    assert_eq!(rig.selection_len(), 1);
}

#[test]
fn shift_or_ctrl_click_toggles_one_item_and_keeps_the_rest() {
    for mods in [Modifiers::SHIFT, Modifiers::CTRL] {
        let mut rig = rig();
        rig.select_all_three();
        rig.click_with(mods, (30.0, 10.0));
        assert_eq!(rig.selected(), set(&[U1, D2]), "{mods:?} takes D1 out");
        rig.click_with(mods, (30.0, 10.0));
        assert_eq!(rig.selected(), set(&[U1, D1, D2]), "{mods:?} puts D1 back");
        rig.click_with(mods, EMPTY);
        assert_eq!(
            rig.selected(),
            set(&[U1, D1, D2]),
            "{mods:?} on empty space keeps the selection"
        );
    }
}

#[test]
fn esc_clears_many() {
    let mut rig = rig();
    rig.select_all_three();
    rig.app.clear_pcb_selection();
    assert_eq!(rig.selection_len(), 0);
}

#[test]
fn a_box_from_empty_space_replaces_and_shift_adds() {
    let mut rig = rig();
    rig.click((10.0, 10.0));
    // Left to right around D1 and D2: a window box.
    rig.drag_box(Modifiers::default(), (26.0, 6.0), (54.0, 14.0));
    assert_eq!(rig.selected(), set(&[D1, D2]), "the box replaces U1");

    rig.drag_box(Modifiers::SHIFT, (6.0, 6.0), (14.0, 14.0));
    assert_eq!(rig.selected(), set(&[U1, D1, D2]), "Shift adds U1");
}

#[test]
fn an_empty_box_clears_unless_shift_or_ctrl() {
    let mut rig = rig();
    rig.select_all_three();
    for mods in [Modifiers::SHIFT, Modifiers::CTRL] {
        rig.drag_box(mods, (26.0, 18.0), (34.0, 26.0));
        assert_eq!(rig.selected(), set(&[U1, D1, D2]), "{mods:?} keeps it");
    }
    rig.drag_box(Modifiers::default(), (26.0, 18.0), (34.0, 26.0));
    assert_eq!(rig.selection_len(), 0, "a plain empty box clears");
}

/// The maintainer's report: a footprint stays selected and the Track tool
/// then places nothing. With U1 selected and Track armed, a click on empty
/// board and a slightly dragged click on U1's pad place one track run, as
/// one undo step, and U1 does not move.
#[test]
fn track_places_with_a_footprint_selected() {
    let mut rig = rig();
    rig.click((10.0, 10.0));
    assert_eq!(rig.selected(), set(&[U1]));
    let before = serde_json::to_string(rig.board()).unwrap();
    let u1_at = rig.board().footprints[0].position;

    let id = AppCommandId::new("place_board_track").expect("a catalog id");
    let _ = rig.app.update(Message::Menu(MenuMessage::Command(id)));
    assert!(
        rig.app.interaction_state.pcb_tool.is_some(),
        "Track is armed"
    );
    assert!(rig.app.interaction_state.pcb_canvas.is_clicks_only());

    // Start on empty board.
    rig.click((20.0, 16.0));
    assert!(
        rig.board().segments.is_empty(),
        "the start adds nothing yet"
    );
    // End on U1's pad 1, the press jiggled past the drag threshold.
    let pad = rig.screen((12.0, 10.0));
    rig.move_to(pad);
    rig.press(pad);
    rig.move_to(iced::Point::new(pad.x + 8.0, pad.y + 6.0));
    rig.release(iced::Point::new(pad.x + 8.0, pad.y + 6.0));

    assert!(!rig.board().segments.is_empty(), "a track run is placed");
    assert_eq!(rig.board().footprints[0].position, u1_at, "U1 did not move");
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        serde_json::to_string(rig.board()).unwrap(),
        before,
        "one undo step takes the run back"
    );
}

/// Arming a tool ends a Move waiting on the cursor, so its clicks are the
/// tool's.
#[test]
fn arming_a_tool_ends_a_pending_move() {
    let mut rig = rig();
    rig.click((10.0, 10.0));
    let id = AppCommandId::new("move_object").expect("a catalog id");
    let _ = rig.app.update(Message::Menu(MenuMessage::Command(id)));
    assert!(rig.app.interaction_state.pcb_carry.is_some(), "Move waits");
    let id = AppCommandId::new("place_board_via").expect("a catalog id");
    let _ = rig.app.update(Message::Menu(MenuMessage::Command(id)));
    assert!(
        rig.app.interaction_state.pcb_carry.is_none(),
        "the Move ended"
    );
    rig.click(EMPTY);
    assert_eq!(rig.board().vias.len(), 1, "the click placed a via");
}
