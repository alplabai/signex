//! #2191 — PCB canvas hover and selection, driven through `Message`s on a
//! real board opened from disk: click, Shift, Ctrl, window vs crossing,
//! Esc, and a selection that outlives zoom and pan.

use iced::Rectangle;
use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{FileMsg, InputMsg, Message, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    Footprint, Pad, PadShape, PadType, PcbBoard, Point, Segment, Via, ViaType,
};
use uuid::Uuid;

const PAD: ItemRef = ItemRef::Pad {
    footprint: Uuid::from_u128(1),
    pad: Uuid::from_u128(2),
};
const TRACK: ItemRef = ItemRef::Track(Uuid::from_u128(3));
const VIA: ItemRef = ItemRef::Via(Uuid::from_u128(4));
/// U1's body (#2240): it has no courtyard, so its one pad's outline.
const FOOTPRINT: ItemRef = ItemRef::Footprint(Uuid::from_u128(1));
const REACH: f64 = 0.1;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// U1 at (10, 10) turned 90° with a 2 x 1 mm pad: it covers x 9.5..10.5,
/// y 11..13. A track (0,0)-(20,0), 0.5 mm wide; a 0.6 mm via at (30, 0).
fn board() -> PcbBoard {
    let pad = Pad {
        uuid: Uuid::from_u128(2),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: pt(2.0, 0.0),
        size: pt(2.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let footprint = Footprint {
        uuid: Uuid::from_u128(1),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: pt(10.0, 10.0),
        rotation: 90.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let segment = Segment {
        uuid: Uuid::from_u128(3),
        start: pt(0.0, 0.0),
        end: pt(20.0, 0.0),
        width: 0.5,
        layer: "TopCopper".into(),
        net: 1,
        unknown_columns: Default::default(),
    };
    let via = Via {
        uuid: Uuid::from_u128(4),
        position: pt(30.0, 0.0),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 1,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint],
        segments: vec![segment],
        vias: vec![via],
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    _tmp: tempfile::TempDir,
}

fn open_board() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path))));
    crate::drive::drive(&mut app, task);
    Fixture { app, _tmp: tmp }
}

fn send(app: &mut Signex, msg: PcbCanvasMsg) {
    let task = app.update(Message::PcbCanvas(msg));
    crate::drive::drive(app, task);
}

fn click(app: &mut Signex, x: f64, y: f64, mode: ClickMode) {
    let msg = PcbCanvasMsg::Clicked {
        at: pt(x, y),
        tolerance_mm: REACH,
        mode,
    };
    send(app, msg);
}

fn drag(app: &mut Signex, from: (f64, f64), to: (f64, f64)) {
    let msg = PcbCanvasMsg::BoxSelected {
        from: pt(from.0, from.1),
        to: pt(to.0, to.1),
        mode: ClickMode::Replace,
    };
    send(app, msg);
}

fn selected(app: &Signex) -> Vec<ItemRef> {
    app.interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

fn escape(app: &mut Signex) {
    let task = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    crate::drive::drive(app, task);
}

#[test]
fn click_selects_the_item_under_the_pointer_and_empty_space_clears() {
    let mut f = open_board();
    click(&mut f.app, 10.0, 12.0, ClickMode::Replace);
    assert_eq!(selected(&f.app), vec![PAD]);
    click(&mut f.app, 5.0, 0.0, ClickMode::Replace);
    assert_eq!(selected(&f.app), vec![TRACK]);
    click(&mut f.app, 5.0, 5.0, ClickMode::Replace);
    assert!(selected(&f.app).is_empty());
}

#[test]
fn shift_adds_and_an_empty_shift_click_changes_nothing() {
    let mut f = open_board();
    click(&mut f.app, 10.0, 12.0, ClickMode::Replace);
    click(&mut f.app, 30.0, 0.0, ClickMode::Add);
    assert_eq!(selected(&f.app), vec![PAD, VIA]);
    click(&mut f.app, 5.0, 5.0, ClickMode::Add);
    assert_eq!(selected(&f.app).len(), 2);
}

#[test]
fn ctrl_toggles_an_item_in_and_out() {
    let mut f = open_board();
    click(&mut f.app, 10.0, 12.0, ClickMode::Replace);
    click(&mut f.app, 5.0, 0.0, ClickMode::Toggle);
    assert_eq!(selected(&f.app), vec![PAD, TRACK]);
    click(&mut f.app, 10.0, 12.0, ClickMode::Toggle);
    assert_eq!(selected(&f.app), vec![TRACK]);
}

#[test]
fn a_window_takes_what_lies_inside_a_crossing_what_it_touches() {
    let mut f = open_board();
    // Left to right: the pad and U1's body are inside, the track is cut by
    // the box.
    drag(&mut f.app, (-1.0, -1.0), (15.0, 14.0));
    assert_eq!(selected(&f.app), vec![PAD, FOOTPRINT]);
    // Right to left over the same box: it touches the track too.
    drag(&mut f.app, (15.0, 14.0), (-1.0, -1.0));
    assert_eq!(selected(&f.app), vec![PAD, TRACK, FOOTPRINT]);
    // A box over nothing replaces the selection with nothing.
    drag(&mut f.app, (40.0, 40.0), (45.0, 45.0));
    assert!(selected(&f.app).is_empty());
}

#[test]
fn a_box_with_shift_adds_to_the_selection() {
    let mut f = open_board();
    click(&mut f.app, 30.0, 0.0, ClickMode::Replace);
    let msg = PcbCanvasMsg::BoxSelected {
        from: pt(-1.0, -1.0),
        to: pt(15.0, 14.0),
        mode: ClickMode::Add,
    };
    send(&mut f.app, msg);
    assert_eq!(selected(&f.app), vec![PAD, VIA, FOOTPRINT]);
}

#[test]
fn hover_follows_the_pointer_and_leaves_the_selection_alone() {
    let mut f = open_board();
    let over = |x, y| PcbCanvasMsg::PointerMoved {
        at: pt(x, y),
        zoom_pct: 100.0,
        tolerance_mm: REACH,
    };
    send(&mut f.app, over(10.0, 12.0));
    assert_eq!(f.app.interaction_state.pcb_selection.hover(), Some(PAD));
    send(&mut f.app, over(5.0, 5.0));
    assert_eq!(f.app.interaction_state.pcb_selection.hover(), None);
    assert!(selected(&f.app).is_empty());
}

#[test]
fn escape_clears_the_selection() {
    let mut f = open_board();
    drag(&mut f.app, (35.0, 14.0), (-1.0, -1.0));
    assert_eq!(selected(&f.app), vec![PAD, TRACK, VIA, FOOTPRINT]);
    escape(&mut f.app);
    assert!(selected(&f.app).is_empty());
}

#[test]
fn the_selection_and_the_index_survive_zoom_and_pan() {
    let mut f = open_board();
    click(&mut f.app, 10.0, 12.0, ClickMode::Replace);
    let builds = f.app.interaction_state.pcb_selection.index_builds();
    assert_eq!(builds, 1);

    let canvas = &f.app.interaction_state.pcb_canvas;
    canvas.set_viewport(Rectangle::new(
        iced::Point::ORIGIN,
        iced::Size::new(800.0, 600.0),
    ));
    assert!(canvas.zoom_about_centre(true, 1.25));
    canvas.center_on_world(20.0, 5.0);
    for event in [CanvasEvent::ViewMoved, CanvasEvent::CursorMoved] {
        let task = f.app.update(Message::CanvasEvent(event));
        crate::drive::drive(&mut f.app, task);
    }

    assert_eq!(selected(&f.app), vec![PAD]);
    assert_eq!(f.app.interaction_state.pcb_selection.index_builds(), builds);
    click(&mut f.app, 5.0, 0.0, ClickMode::Add);
    assert_eq!(f.app.interaction_state.pcb_selection.index_builds(), builds);
}
