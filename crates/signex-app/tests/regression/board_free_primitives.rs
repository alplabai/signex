//! #2373, #2393: a board's free arcs, pads, strings and dimensions, driven
//! through `Signex::update`. They load from the `.snxpcb` `[free]` section,
//! a click selects each, its Properties panel shows it, and every edit is
//! one board undo step. Placing them is covered by the app's
//! `handlers::pcb_tool::free_tests`.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget,
};
use signex_app::panels::{PanelMsg, PcbPrimitiveMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::test_adapter::BoardEngines;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

const ARC: Uuid = Uuid::from_u128(0x401);
const PAD: Uuid = Uuid::from_u128(0x402);
const DIM: Uuid = Uuid::from_u128(0x403);
const TEXT: Uuid = Uuid::from_u128(0x404);

/// A half circle of radius 5 round (20, 20) on top copper, a free pad on
/// net 3 at (5, 5), a 10 mm dimension along y = 10 and a string at (5, 30).
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3},
        "nets": [{"number": 3, "name": "GND"}],
        "texts": [{"uuid": TEXT, "text": "A", "position": {"x": 5.0, "y": 30.0},
                   "layer": "TopSilkscreen", "font_size": 1.0, "rotation": 0.0}],
        "free": {
            "arcs": [{"uuid": ARC, "start": {"x": 25.0, "y": 20.0},
                      "mid": {"x": 20.0, "y": 25.0}, "end": {"x": 15.0, "y": 20.0},
                      "width": 0.3, "layer": "TopCopper", "net": 0}],
            "pads": [{"uuid": PAD, "number": "1", "pad_type": "thru", "shape": "circle",
                      "position": {"x": 5.0, "y": 5.0}, "size": {"x": 1.5, "y": 1.5},
                      "drill": {"diameter": 0.8}, "layers": ["TopCopper", "BottomCopper"],
                      "net": {"number": 3, "name": "GND"}}],
            "dimensions": [{"uuid": DIM, "start": {"x": 30.0, "y": 10.0},
                            "end": {"x": 40.0, "y": 10.0}, "offset": 2.0,
                            "layer": "Dimensions", "line_width": 0.1,
                            "text_height": 1.0, "precision": 2,
                            // The point a placed dimension carries: Above,
                            // half a text height off the line (#2868).
                            "text_at": {"x": 0.0, "y": 1.0}}]
        }
    }))
    .unwrap()
}

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn open_board() -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    Fixture {
        app,
        path,
        _tmp: tmp,
    }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn live(app: &Signex) -> PcbBoard {
    let docs = &app.document_state;
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn click(app: &mut Signex, at: Point) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at,
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
}

fn facts(app: &Signex) -> PcbPrimitiveFacts {
    app.document_view
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.clone())
        .expect("the primitive panel")
}

fn panel(msg: PcbPrimitiveMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(msg)))
}

fn target(f: &Fixture, item: ItemRef) -> PrimitiveTarget {
    PrimitiveTarget {
        board: f.path.clone(),
        items: vec![item],
    }
}

fn type_and_enter(f: &mut Fixture, target: &PrimitiveTarget, field: PrimitiveField, text: &str) {
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field,
            text: text.into(),
        }),
    );
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        }),
    );
}

/// The edit `run` makes is one undo step: one undo restores the board,
/// redo replays it, and a second undo leaves nothing of it.
fn assert_one_step(f: &mut Fixture, run: impl FnOnce(&mut Fixture)) -> PcbBoard {
    let before = snapshot(&f.app);
    run(f);
    let after = snapshot(&f.app);
    assert_ne!(before, after, "the board did not change");
    let edited = live(&f.app);
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "one undo restores the board");
    send(&mut f.app, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(&f.app), after, "redo replays it");
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "it was a single step");
    edited
}

#[test]
fn the_free_section_loads_with_the_board() {
    let f = open_board();
    let loaded = live(&f.app);
    assert_eq!(loaded.free, board().free, "loaded as saved");
    assert_eq!(loaded.texts.len(), 1);
}

#[test]
fn a_clicked_arc_shows_the_arc_panel_and_its_width_is_one_step() {
    let mut f = open_board();
    click(&mut f.app, Point::new(25.0, 20.0));
    let PcbPrimitiveFacts::Arc(arc) = facts(&f.app) else {
        panic!("the arc panel shows");
    };
    let t = target(&f, ItemRef::Arc(ARC));
    assert_eq!(arc.common.target, t);
    let radius = arc.radius.expect("one arc, one radius");
    assert!((radius - 5.0).abs() < 1e-9, "{radius}");
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::Width, "0.5");
    });
    assert_eq!(edited.free.arcs[0].width, 0.5);
}

#[test]
fn a_clicked_free_pad_shows_the_pad_panel_and_its_size_is_one_step() {
    let mut f = open_board();
    click(&mut f.app, Point::new(5.0, 5.0));
    let PcbPrimitiveFacts::Pad(pad) = facts(&f.app) else {
        panic!("a free pad shows the pad panel");
    };
    let t = target(&f, ItemRef::FreePad(PAD));
    assert_eq!(pad.common.target, t);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::SizeX, "2.0");
    });
    assert_eq!(edited.free.pads[0].size.x, 2.0);
}

#[test]
fn a_clicked_dimension_shows_its_value_and_its_precision_is_one_step() {
    let mut f = open_board();
    let drawing = board().free.dimensions[0].drawing().expect("a drawing");
    let (a, b) = drawing.strokes[0];
    click(&mut f.app, Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
    let PcbPrimitiveFacts::Dimension(dimension) = facts(&f.app) else {
        panic!("the dimension panel shows");
    };
    let t = target(&f, ItemRef::Dimension(DIM));
    assert_eq!(dimension.common.target, t);
    assert_eq!(dimension.value.as_deref(), Some("10.00mm"));
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::Precision, "1");
    });
    assert_eq!(edited.free.dimensions[0].value_text(), "10.0mm");
}

#[test]
fn a_clicked_string_shows_the_string_panel_and_its_text_is_one_step() {
    let mut f = open_board();
    click(&mut f.app, Point::new(5.0, 30.0));
    let PcbPrimitiveFacts::Text(text) = facts(&f.app) else {
        panic!("the string panel shows");
    };
    let t = target(&f, ItemRef::Text(TEXT));
    assert_eq!(text.common.target, t);
    assert_eq!(text.text.as_deref(), Some("A"));
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::Text, "REV B");
    });
    assert_eq!(edited.texts[0].text, "REV B");
}

#[test]
fn the_place_rows_arm_their_tools() {
    let mut f = open_board();
    for command in [
        "place_board_pad",
        "place_board_arc",
        "place_board_string",
        "place_board_dimension",
    ] {
        let id = AppCommandId::new(command).expect("a catalog id");
        send(&mut f.app, Message::Menu(MenuMessage::Command(id)));
        let form = facts(&f.app);
        assert!(
            matches!(
                form,
                PcbPrimitiveFacts::Pad(_)
                    | PcbPrimitiveFacts::Arc(_)
                    | PcbPrimitiveFacts::Text(_)
                    | PcbPrimitiveFacts::Dimension(_)
            ),
            "{command} shows its placement form"
        );
        assert!(form.common().target.is_placement(), "{command}");
    }
}
