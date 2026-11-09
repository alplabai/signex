//! #2240 — placing footprints on a board from the UI: the rotate key, an
//! Edit menu row, a context-menu row, an Active Bar action, a drag and the
//! Delete key each edit the board as one undo step, with the pads' world
//! shapes following; a locked footprint refuses with a notice.

use pcb_geom::{ClickMode, ItemRef, PointNm, Shape, pad_shape};
use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{
    ContextAction, ContextMenuMsg, EditMsg, FileMsg, Message, Signex, TabDocument, UiMsg,
};
use signex_app::diagnostics;
use signex_app::keymap::{AppCommandId, KeyStroke, KeyToken, Modifiers};
use signex_app::menu_bar::MenuMessage;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    Footprint, Pad, PadShape, PadType, PcbBoard, Point, Segment, Via, ViaType,
};
use uuid::Uuid;

const U1: Uuid = Uuid::from_u128(0x10);
const U2: Uuid = Uuid::from_u128(0x20);
const ROUND_RECT: Uuid = Uuid::from_u128(0x11);
const OVAL: Uuid = Uuid::from_u128(0x12);
/// A track (0, 20)-(10, 20) and a via at (20, 20), clear of both parts.
const TRACK: Uuid = Uuid::from_u128(0x30);
const VIA: Uuid = Uuid::from_u128(0x31);
const REACH: f64 = 0.05;

fn pad(uuid: Uuid, shape: PadShape, at: (f64, f64), size: (f64, f64), rotation: f64) -> Pad {
    Pad {
        uuid,
        number: "1".into(),
        pad_type: PadType::Smd,
        shape,
        position: Point::new(at.0, at.1),
        size: Point::new(size.0, size.1),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.25,
        rotation,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(uuid: Uuid, reference: &str, x: f64, locked: bool, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid,
        reference: reference.into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(x, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked,
        schematic_link: None,
        library_ref: None,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// U1 at (10, 10): a round-rect pad at (2, 0) turned 30° and an oval pad at
/// (-2, 0). U2, locked, at (30, 10).
fn board() -> PcbBoard {
    let u1 = footprint(
        U1,
        "U1",
        10.0,
        false,
        vec![
            pad(
                ROUND_RECT,
                PadShape::RoundRect,
                (2.0, 0.0),
                (1.0, 0.6),
                30.0,
            ),
            pad(OVAL, PadShape::Oval, (-2.0, 0.0), (1.6, 0.8), 0.0),
        ],
    );
    let u2 = footprint(
        U2,
        "U2",
        30.0,
        true,
        vec![pad(
            Uuid::from_u128(0x21),
            PadShape::Rect,
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )],
    );
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
        footprints: vec![u1, u2],
        segments: vec![Segment {
            uuid: TRACK,
            start: Point::new(0.0, 20.0),
            end: Point::new(10.0, 20.0),
            width: 0.5,
            layer: "TopCopper".into(),
            net: 1,
            unknown_columns: Default::default(),
        }],
        vias: vec![Via {
            uuid: VIA,
            position: Point::new(20.0, 20.0),
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 1,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        }],
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
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    let task = app.update(Message::File(FileMsg::Opened(Some(path))));
    crate::drive::drive(&mut app, task);
    Fixture { app, _tmp: tmp }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn live(app: &Signex) -> PcbBoard {
    let docs = &app.document_state;
    docs.tabs[docs.active_tab]
        .cached_document
        .as_ref()
        .and_then(TabDocument::as_pcb)
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn fp(board: &PcbBoard, id: Uuid) -> &Footprint {
    board
        .footprints
        .iter()
        .find(|f| f.uuid == id)
        .expect("footprint")
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let at = Point::new(x, y);
    let mode = ClickMode::Replace;
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at,
            tolerance_mm: REACH,
            mode,
        }),
    );
}

/// Select U1 by its body, between its pads.
fn select_u1(app: &mut Signex) {
    click(app, 10.0, 10.0);
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, vec![ItemRef::Footprint(U1)]);
}

fn key(token: KeyToken, shift: bool) -> Message {
    let modifiers = Modifiers {
        shift,
        ..Modifiers::default()
    };
    Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: KeyStroke {
            modifiers,
            key: token,
        },
    })
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn undo(app: &mut Signex) {
    send(app, Message::Edit(EditMsg::Undo));
}

fn redo(app: &mut Signex) {
    send(app, Message::Edit(EditMsg::Redo));
}

/// `message` edits the board once; one undo puts it back bit for bit and
/// redo replays it. Returns the edited board.
fn one_step(f: &mut Fixture, message: Message) -> PcbBoard {
    let before = snapshot(&f.app);
    send(&mut f.app, message);
    let after = snapshot(&f.app);
    assert_ne!(before, after, "the board did not change");
    let edited = live(&f.app);
    undo(&mut f.app);
    assert_eq!(snapshot(&f.app), before, "one undo restores the board");
    redo(&mut f.app);
    assert_eq!(snapshot(&f.app), after, "redo replays it");
    undo(&mut f.app);
    undo(&mut f.app);
    assert_eq!(snapshot(&f.app), before, "it was a single step");
    edited
}

fn mm(x: f64, y: f64) -> PointNm {
    PointNm::from_mm(Point::new(x, y), "test").unwrap()
}

fn pad_world(board: &PcbBoard, pad: Uuid) -> Shape {
    let u1 = fp(board, U1);
    let pad = u1.pads.iter().find(|p| p.uuid == pad).unwrap();
    pad_shape(u1, pad).unwrap()
}

fn round_rect_centre_and_turn(shape: Shape) -> (PointNm, f64) {
    match shape {
        Shape::RoundRect {
            center,
            rotation_deg,
            ..
        } => (center, rotation_deg),
        other => panic!("the round-rect pad lost its shape: {other:?}"),
    }
}

#[test]
fn the_rotate_key_turns_the_footprint_and_its_pads_clockwise() {
    let mut f = open_board();
    select_u1(&mut f.app);
    // Altium: Shift+Space turns clockwise; the board's y axis points down.
    let edited = one_step(&mut f, key(KeyToken::Space, true));
    let u1 = fp(&edited, U1);
    assert_eq!(u1.rotation, 90.0);
    assert_eq!(
        u1.position,
        Point::new(10.0, 10.0),
        "a lone part turns in place"
    );
    let (centre, turn) = round_rect_centre_and_turn(pad_world(&edited, ROUND_RECT));
    assert_eq!(centre, mm(10.0, 12.0));
    assert_eq!(turn, 120.0);
    assert_eq!(
        pad_world(&edited, OVAL),
        Shape::Capsule {
            a: mm(10.0, 7.6),
            b: mm(10.0, 8.4),
            radius: mm(0.4, 0.0).x,
        }
    );
}

#[test]
fn the_edit_menu_rotate_row_turns_counter_clockwise() {
    let mut f = open_board();
    select_u1(&mut f.app);
    let row = Message::Menu(MenuMessage::Command(command("rotate_counterclockwise")));
    let edited = one_step(&mut f, row);
    assert_eq!(fp(&edited, U1).rotation, 270.0);
    let (centre, turn) = round_rect_centre_and_turn(pad_world(&edited, ROUND_RECT));
    assert_eq!(centre, mm(10.0, 8.0));
    assert_eq!(turn, 300.0);
}

#[test]
fn the_context_menu_flip_row_puts_the_footprint_on_the_bottom() {
    let mut f = open_board();
    select_u1(&mut f.app);
    let row = Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
        id: command("flip_selection"),
        project: None,
    }));
    let edited = one_step(&mut f, row);
    let u1 = fp(&edited, U1);
    assert_eq!(u1.layer, "BottomCopper");
    assert!(u1.pads.iter().all(|p| p.layers == ["BottomCopper"]));
    // Mirrored left to right: the round-rect pad is now left of the origin
    // and its own 30° turn is reversed.
    let (centre, turn) = round_rect_centre_and_turn(pad_world(&edited, ROUND_RECT));
    assert_eq!(centre, mm(8.0, 10.0));
    assert_eq!(turn, -30.0);
}

#[test]
fn the_x_key_and_the_active_bar_flip_agree_and_y_flips_top_to_bottom() {
    let mut by_key = open_board();
    select_u1(&mut by_key.app);
    let keyed = one_step(&mut by_key, key(KeyToken::Character("x".into()), false));

    let mut by_bar = open_board();
    select_u1(&mut by_bar.app);
    let bar = Message::ActiveBar(ActiveBarMsg::Action(ActiveBarAction::FlipSelectedX));
    let barred = one_step(&mut by_bar, bar);
    assert_eq!(fp(&keyed, U1), fp(&barred, U1));
    assert_eq!(fp(&keyed, U1).layer, "BottomCopper");

    let mut top_bottom = open_board();
    select_u1(&mut top_bottom.app);
    let bar = Message::ActiveBar(ActiveBarMsg::Action(ActiveBarAction::FlipSelectedY));
    let edited = one_step(&mut top_bottom, bar);
    let u1 = fp(&edited, U1);
    assert_eq!((u1.layer.as_str(), u1.rotation), ("BottomCopper", 180.0));
    // Top to bottom: the pad keeps its x side and its turn mirrors.
    let (centre, _) = round_rect_centre_and_turn(pad_world(&edited, ROUND_RECT));
    assert_eq!(centre, mm(12.0, 10.0));
}

#[test]
fn a_drag_previews_then_drops_the_footprint_on_the_grid() {
    let mut f = open_board();
    select_u1(&mut f.app);
    let before = snapshot(&f.app);
    let (from, to) = (Point::new(10.0, 10.0), Point::new(13.1, 10.2));
    send(
        &mut f.app,
        Message::PcbCanvas(PcbCanvasMsg::FootprintDragged { from, to }),
    );
    assert_eq!(snapshot(&f.app), before, "a drag in progress edits nothing");
    let ghost = &f.app.interaction_state.pcb_canvas.highlight().ghost;
    assert_eq!(ghost.len(), 3, "the body and both pads are previewed");
    let dropped = Message::PcbCanvas(PcbCanvasMsg::FootprintDropped { from, to });
    let edited = one_step(&mut f, dropped);
    // The origin snaps to the 1.27 mm PCB default grid: (12.7, 10.16).
    let at = fp(&edited, U1).position;
    assert!(
        (at.x - 12.7).abs() < 1e-9 && (at.y - 10.16).abs() < 1e-9,
        "{at:?}"
    );
    assert!(
        f.app
            .interaction_state
            .pcb_canvas
            .highlight()
            .ghost
            .is_empty()
    );
}

#[test]
fn the_delete_key_removes_the_footprint_as_one_step() {
    let mut f = open_board();
    select_u1(&mut f.app);
    let edited = one_step(&mut f, key(KeyToken::Delete, false));
    assert!(edited.footprints.iter().all(|f| f.uuid != U1));
    assert_eq!(edited.footprints.len(), 1);
}

#[test]
fn a_locked_footprint_refuses_every_edit_with_a_notice() {
    let mut f = open_board();
    click(&mut f.app, 30.0, 10.0);
    assert!(
        f.app
            .interaction_state
            .pcb_selection
            .selection()
            .footprints()
            .contains(&U2),
        "the pad selects its locked footprint"
    );
    let before = snapshot(&f.app);
    let attempts = [
        ("Rotate", key(KeyToken::Space, false)),
        ("Flip", key(KeyToken::Character("x".into()), false)),
        ("Delete", key(KeyToken::Delete, false)),
        (
            "Move",
            Message::PcbCanvas(PcbCanvasMsg::FootprintDropped {
                from: Point::new(30.0, 10.0),
                to: Point::new(35.0, 10.0),
            }),
        ),
    ];
    // The diagnostics ring is shared by every test thread and holds a few
    // hundred lines, so each refusal is looked for right after its own
    // attempt and by its own operation name, not counted at the end (#2227).
    for (operation, attempt) in attempts {
        let since = diagnostics::newest_entry_id().unwrap_or(0);
        send(&mut f.app, attempt);
        assert_eq!(snapshot(&f.app), before, "a locked footprint stays put");
        let wanted = format!("{operation}: U2 is locked");
        assert!(
            diagnostics::recent_entries()
                .iter()
                .any(|e| e.id > since && e.message.contains(&wanted)),
            "the {operation} refusal names the locked footprint"
        );
    }
    assert!(!can_undo(&f.app), "no undo step was made");
}

fn can_undo(app: &Signex) -> bool {
    let docs = &app.document_state;
    docs.tabs[docs.active_tab]
        .cached_document
        .as_ref()
        .and_then(TabDocument::as_pcb_engine)
        .expect("a board engine")
        .can_undo()
}

fn select_add(app: &mut Signex, x: f64, y: f64) {
    let msg = PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: REACH,
        mode: ClickMode::Add,
    };
    send(app, Message::PcbCanvas(msg));
}

#[test]
fn delete_removes_a_selected_track_and_via_as_one_step() {
    let mut f = open_board();
    click(&mut f.app, 5.0, 20.0);
    select_add(&mut f.app, 20.0, 20.0);
    let selected: Vec<ItemRef> = f
        .app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, vec![ItemRef::Track(TRACK), ItemRef::Via(VIA)]);
    let edited = one_step(&mut f, key(KeyToken::Delete, false));
    assert!(edited.segments.is_empty() && edited.vias.is_empty());
    assert_eq!(edited.footprints.len(), 2, "no footprint went with them");
}

#[test]
fn delete_with_a_locked_footprint_in_the_selection_removes_nothing() {
    let mut f = open_board();
    click(&mut f.app, 20.0, 20.0);
    select_add(&mut f.app, 30.0, 10.0);
    let before = snapshot(&f.app);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    send(&mut f.app, key(KeyToken::Delete, false));
    assert_eq!(snapshot(&f.app), before, "the via stays with the locked U2");
    let noticed = diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > since && e.message.contains("Delete: U2 is locked"));
    assert!(noticed, "the refusal names U2");
    assert!(!can_undo(&f.app));
}
